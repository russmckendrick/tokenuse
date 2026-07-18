<script lang="ts">
  import { onMount } from 'svelte';
  import ForceGraph3D, {
    type ForceGraph3DInstance,
    type LinkObject,
    type NodeObject
  } from '3d-force-graph';
  import { forceX, forceY, scaleSqrt, type ForceLink } from 'd3';
  import {
    AmbientLight,
    CanvasTexture,
    Color,
    CylinderGeometry,
    DirectionalLight,
    GridHelper,
    Group,
    Mesh,
    MeshBasicMaterial,
    MeshStandardMaterial,
    OctahedronGeometry,
    SphereGeometry,
    Sprite,
    SpriteMaterial,
    SRGBColorSpace,
    TorusGeometry,
    type BufferGeometry,
    type Material,
    type Object3D
  } from 'three';
  import { count } from '../format';
  import { providerMark } from '../icons';
  import type { GraphCamera, GraphLens } from '../lib/graph.svelte';
  import type {
    CopyDeck,
    GraphEdge,
    GraphMetricId,
    GraphNode,
    GraphNodeKind,
    GraphRelation
  } from '../types';

  export let nodes: GraphNode[] = [];
  export let edges: GraphEdge[] = [];
  export let lens: GraphLens = 'projects';
  export let metric: GraphMetricId = 'calls';
  export let selectedId: string | null = null;
  export let camera: GraphCamera | null = null;
  export let copy: CopyDeck['graph'];
  export let selectNode: (id: string | null) => void;
  export let saveCamera: (camera: GraphCamera | null) => void;

  type SceneNode = GraphNode &
    NodeObject & {
      radius: number;
      value: number;
      x: number;
      y: number;
      z: number;
    };

  type SceneLink = Omit<GraphEdge, 'source' | 'target'> &
    LinkObject<SceneNode> & {
      source: string | SceneNode;
      target: string | SceneNode;
      value: number;
    };

  type OrbitControlsLike = {
    target: { x: number; y: number; z: number };
    enableDamping: boolean;
    dampingFactor: number;
    minDistance: number;
    maxDistance: number;
    addEventListener: (name: string, callback: () => void) => void;
    removeEventListener: (name: string, callback: () => void) => void;
  };

  type NodeVisual = {
    group: Group;
    bodyMaterial: MeshStandardMaterial;
    halo: Group;
    haloMaterial: MeshBasicMaterial;
    label: Sprite;
    labelMaterial: SpriteMaterial;
    iconMaterial: SpriteMaterial | null;
  };

  type SceneForce = {
    (alpha: number): void;
    initialize: (nodes: SceneNode[]) => void;
  };

  let container: HTMLDivElement;
  let graph: ForceGraph3DInstance<SceneNode, SceneLink> | null = null;
  let controls: OrbitControlsLike | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let sceneNodes: SceneNode[] = [];
  let sceneLinks: SceneLink[] = [];
  let guideGroup: Group | null = null;
  let renderedToken = '';
  let hoveredId: string | null = null;
  let needsInitialFit = false;
  let needsVisualSync = false;
  let skipCameraRestore = false;
  let layoutRevision = 0;
  let reduceMotion = false;
  const nodeVisuals = new Map<string, NodeVisual>();
  const colors = new Map<string, string>();
  const persistentLabelIds = new Set<string>();

  $: sceneToken = [
    lens,
    metric,
    layoutRevision,
    nodes.map((node) => `${node.id}:${metricValue(node)}`).join(','),
    edges.map((edge) => `${edge.id}:${metricValue(edge)}`).join(',')
  ].join('|');
  $: selectedAriaNode = selectedId
    ? nodes.find((node) => node.id === selectedId) ?? null
    : null;

  $: if (graph && sceneToken !== renderedToken) {
    renderedToken = sceneToken;
    rebuildGraph();
  }

  $: if (graph) {
    selectedId;
    updateVisualState();
  }

  onMount(() => {
    reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    readColors();
    const bounds = container.getBoundingClientRect();
    const nextGraph = new ForceGraph3D(container, {
      controlType: 'orbit',
      rendererConfig: {
        antialias: true,
        alpha: false,
        powerPreference: 'high-performance'
      }
    }) as unknown as ForceGraph3DInstance<SceneNode, SceneLink>;
    graph = nextGraph;
    nextGraph
      .width(Math.max(320, Math.round(bounds.width)))
      .height(Math.max(500, Math.round(bounds.height)))
      .backgroundColor(token('--color-surface-sunken'))
      .showNavInfo(false)
      .nodeId('id')
      .nodeThreeObject((node) => makeNodeObject(node))
      .nodeThreeObjectExtend(false)
      .nodeLabel((node) => nodeTooltip(node))
      .linkLabel((link) => linkTooltip(link))
      .linkColor((link) => linkColor(link))
      .linkWidth((link) => linkWidth(link))
      .linkOpacity(0.42)
      .linkCurvature((link) => linkCurvature(link))
      .linkResolution(4)
      .enableNodeDrag(true)
      .enableNavigationControls(true)
      .onNodeClick((node) => selectNode(node.id))
      .onNodeHover((node) => {
        hoveredId = node?.id ?? null;
        updateVisualState();
      })
      .onBackgroundClick(() => selectNode(null))
      .onEngineTick(handleEngineFrame)
      .onEngineStop(handleEngineFrame);

    const ambient = new AmbientLight(token('--color-on-surface'), 1.5);
    const key = new DirectionalLight(token('--color-on-surface'), 2.25);
    key.position.set(-120, 180, 240);
    nextGraph.lights([ambient, key]);
    nextGraph.renderer().setPixelRatio(Math.min(window.devicePixelRatio, 2));

    controls = nextGraph.controls() as OrbitControlsLike;
    controls.enableDamping = !reduceMotion;
    controls.dampingFactor = 0.075;
    controls.minDistance = 55;
    controls.maxDistance = 1_100;
    controls.addEventListener('end', persistCamera);

    resizeObserver = new ResizeObserver(([entry]) => {
      if (!graph) return;
      const width = Math.max(320, Math.round(entry.contentRect.width));
      const height = Math.max(500, Math.round(entry.contentRect.height));
      graph.width(width).height(height);
    });
    resizeObserver.observe(container);

    renderedToken = sceneToken;
    rebuildGraph();

    return () => {
      resizeObserver?.disconnect();
      controls?.removeEventListener('end', persistCamera);
      removeGuides();
      graph?._destructor();
      graph = null;
      controls = null;
    };
  });

  function readColors() {
    for (const name of [
      '--color-primary',
      '--color-secondary',
      '--color-tertiary',
      '--color-cyan',
      '--color-magenta',
      '--color-muted',
      '--color-muted-2',
      '--color-on-surface',
      '--color-border',
      '--color-border-soft',
      '--color-surface-sunken'
    ]) {
      colors.set(name, getComputedStyle(container).getPropertyValue(name).trim());
    }
  }

  function token(name: string): string {
    return colors.get(name) || getComputedStyle(container).getPropertyValue(name).trim();
  }

  function metricValue(item: GraphNode | GraphEdge): number {
    if (metric === 'spend') return item.stats.cost_value;
    if (metric === 'tokens') return item.stats.tokens;
    return item.stats.calls;
  }

  function anchor(kind: GraphNodeKind): boolean {
    return lens === 'projects'
      ? kind === 'project' || kind === 'tool'
      : kind === 'tool' || kind === 'model';
  }

  function kindColor(kind: GraphNodeKind): string {
    if (kind === 'project') return token('--color-tertiary');
    if (kind === 'tool') return token('--color-primary');
    if (kind === 'model') return token('--color-magenta');
    if (kind === 'core_tool') return token('--color-cyan');
    return token('--color-secondary');
  }

  function relationColor(relation: GraphRelation): string {
    if (relation === 'project_tool') return token('--color-primary');
    if (relation === 'project_model') return token('--color-magenta');
    if (relation === 'tool_model') return token('--color-cyan');
    if (relation.includes('mcp')) return token('--color-secondary');
    return token('--color-tertiary');
  }

  function subduedRelationColor(relation: GraphRelation): string {
    return new Color(relationColor(relation))
      .lerp(new Color(token('--color-surface-sunken')), 0.38)
      .getStyle();
  }

  function inactiveRelationColor(relation: GraphRelation): string {
    return new Color(relationColor(relation))
      .lerp(new Color(token('--color-border-soft')), 0.76)
      .getStyle();
  }

  function kindLabel(kind: GraphNodeKind): string {
    if (kind === 'project') return copy.kind_project;
    if (kind === 'tool') return copy.kind_tool;
    if (kind === 'model') return copy.kind_model;
    if (kind === 'core_tool') return copy.kind_core_tool;
    return copy.kind_mcp_server;
  }

  function relationLabel(relation: GraphRelation): string {
    if (relation === 'project_tool') return copy.relation_project_tool;
    if (relation === 'project_model') return copy.relation_project_model;
    if (relation === 'tool_model') return copy.relation_tool_model;
    if (relation === 'project_core_tool') return copy.relation_project_core_tool;
    if (relation === 'tool_core_tool') return copy.relation_tool_core_tool;
    if (relation === 'project_mcp_server') return copy.relation_project_mcp_server;
    return copy.relation_tool_mcp_server;
  }

  function hash(value: string, salt = 0): number {
    let output = 2_166_136_261 ^ salt;
    for (let index = 0; index < value.length; index += 1) {
      output ^= value.charCodeAt(index);
      output = Math.imul(output, 16_777_619);
    }
    return output >>> 0;
  }

  function unit(value: string, salt: number): number {
    return (hash(value, salt) % 10_000) / 9_999;
  }

  function xTarget(kind: GraphNodeKind): number {
    if (lens === 'projects') {
      if (kind === 'project') return -170;
      if (kind === 'tool') return 0;
      if (kind === 'model') return 170;
      return 18;
    }
    if (kind === 'tool') return -170;
    if (kind === 'model') return 0;
    if (kind === 'project') return 170;
    return 18;
  }

  function zTarget(kind: GraphNodeKind): number {
    if (kind === 'core_tool' || kind === 'mcp_server') return 125;
    if (lens === 'projects') {
      if (kind === 'tool') return -58;
      if (kind === 'model') return 46;
      return 12;
    }
    if (kind === 'tool') return -62;
    if (kind === 'model') return 18;
    return 72;
  }

  function yTarget(node: GraphNode): number {
    return (unit(node.id, 7) - 0.5) * 230;
  }

  function zForce(): SceneForce {
    let forceNodes: SceneNode[] = [];
    const force = ((alpha: number) => {
      for (const node of forceNodes) {
        const velocity = node.vz ?? 0;
        node.vz = velocity + (zTarget(node.kind) - node.z) * 0.16 * alpha;
      }
    }) as SceneForce;
    force.initialize = (nextNodes) => {
      forceNodes = nextNodes;
    };
    return force;
  }

  function makeBody(kind: GraphNodeKind, radius: number, material: MeshStandardMaterial): Group {
    const body = new Group();
    const add = (geometry: BufferGeometry, setup?: (mesh: Mesh) => void) => {
      const mesh = new Mesh(geometry, material);
      setup?.(mesh);
      body.add(mesh);
      return mesh;
    };

    if (kind === 'project') {
      add(new CylinderGeometry(radius, radius, radius * 0.3, 32), (mesh) => {
        mesh.rotation.x = Math.PI / 2;
      });
      add(new TorusGeometry(radius * 0.82, radius * 0.055, 8, 32), (mesh) => {
        mesh.position.z = radius * 0.17;
      });
      body.rotation.y = -0.13;
    } else if (kind === 'tool') {
      add(new CylinderGeometry(radius * 0.96, radius * 0.96, radius * 0.58, 6), (mesh) => {
        mesh.rotation.x = Math.PI / 2;
      });
      add(new CylinderGeometry(radius * 0.48, radius * 0.48, radius * 0.66, 6), (mesh) => {
        mesh.rotation.x = Math.PI / 2;
      });
      body.rotation.set(-0.08, 0.2, 0.08);
    } else if (kind === 'model') {
      add(new SphereGeometry(radius * 0.56, 20, 14));
      add(new TorusGeometry(radius * 0.87, radius * 0.11, 10, 38));
      add(new TorusGeometry(radius * 0.69, radius * 0.045, 7, 30), (mesh) => {
        mesh.rotation.set(0.72, 0.28, 0);
      });
      body.rotation.set(0.08, -0.15, 0.05);
    } else if (kind === 'core_tool') {
      add(new OctahedronGeometry(radius * 0.78, 0));
      add(new TorusGeometry(radius * 0.8, radius * 0.05, 7, 28), (mesh) => {
        mesh.rotation.set(0.62, 0.25, 0);
      });
    } else {
      add(new TorusGeometry(radius * 0.76, radius * 0.16, 10, 34));
      add(new SphereGeometry(radius * 0.24, 14, 10));
      for (const direction of [-1, 1]) {
        add(new SphereGeometry(radius * 0.13, 12, 8), (mesh) => {
          mesh.position.x = direction * radius * 0.76;
        });
      }
      body.rotation.set(0.12, 0.18, 0);
    }
    return body;
  }

  function compactLabel(node: GraphNode): string {
    const limit = node.kind === 'project' ? 24 : 28;
    if (node.label.length <= limit) return node.label;
    const tailLength = Math.max(7, Math.floor(limit * 0.42));
    const headLength = limit - tailLength - 1;
    return `${node.label.slice(0, headLength)}…${node.label.slice(-tailLength)}`;
  }

  function labelDirection(node: GraphNode): number {
    const target = xTarget(node.kind);
    if (target < -80) return 1;
    if (target > 80) return -1;
    return unit(node.id, 41) < 0.5 ? -1 : 1;
  }

  function makeTextSprite(
    text: string,
    color: string,
    opacity = 1,
    withScrim = false
  ): Sprite {
    const canvas = document.createElement('canvas');
    const measure = canvas.getContext('2d');
    const font = '600 22px Inter, system-ui, sans-serif';
    if (!measure) return new Sprite();
    measure.font = font;
    const textWidth = Math.ceil(measure.measureText(text).width);
    const logicalWidth = Math.max(56, textWidth + 24);
    const logicalHeight = 38;
    const resolution = 2;
    canvas.width = logicalWidth * resolution;
    canvas.height = logicalHeight * resolution;
    const context = canvas.getContext('2d');
    if (!context) return new Sprite();
    context.scale(resolution, resolution);
    if (withScrim) {
      context.globalAlpha = 0.82;
      context.fillStyle = token('--color-surface-sunken');
      context.fillRect(0, 2, logicalWidth, logicalHeight - 4);
    }
    context.globalAlpha = 1;
    context.font = font;
    context.fillStyle = color;
    context.textBaseline = 'middle';
    context.fillText(text, 12, logicalHeight / 2);
    const texture = new CanvasTexture(canvas);
    texture.colorSpace = SRGBColorSpace;
    texture.needsUpdate = true;
    const material = new SpriteMaterial({
      map: texture,
      transparent: true,
      opacity,
      depthTest: false,
      depthWrite: false
    });
    const sprite = new Sprite(material);
    sprite.scale.set(logicalWidth * 0.18, logicalHeight * 0.18, 1);
    sprite.renderOrder = 20;
    return sprite;
  }

  function makeUnknownModelSprite(radius: number): Sprite {
    const canvas = window.document.createElement('canvas');
    const textureSize = 256;
    const padding = 32;
    canvas.width = textureSize;
    canvas.height = textureSize;
    const context = canvas.getContext('2d');
    if (!context) return new Sprite();
    const scale = (textureSize - padding * 2) / 24;
    context.translate(padding, padding);
    context.scale(scale, scale);
    context.strokeStyle = token('--color-on-surface');
    context.lineWidth = 2;
    context.lineCap = 'round';
    context.lineJoin = 'round';
    context.globalAlpha = 0.94;
    context.stroke(new Path2D('M9.09 9a3 3 0 1 1 5.83 1c0 2-3 3-3 3'));
    context.stroke(new Path2D('M12 17h.01'));

    return makeIconSprite(canvas, radius, 0.82);
  }

  function makeProviderSprite(provider: string, radius: number): Sprite {
    const mark = providerMark(provider);
    if (!mark) return makeUnknownModelSprite(radius);
    const document = new DOMParser().parseFromString(mark, 'image/svg+xml');
    const svg = document.documentElement;
    const viewBox = (svg.getAttribute('viewBox') ?? '0 0 24 24')
      .split(/\s+/)
      .map(Number);
    const [minX, minY, width, height] = viewBox;
    if (![minX, minY, width, height].every(Number.isFinite) || width <= 0 || height <= 0) {
      return makeUnknownModelSprite(radius);
    }

    const canvas = window.document.createElement('canvas');
    const textureSize = 256;
    const padding = 20;
    canvas.width = textureSize;
    canvas.height = textureSize;
    const context = canvas.getContext('2d');
    if (!context) return makeUnknownModelSprite(radius);
    const scale = Math.min(
      (textureSize - padding * 2) / width,
      (textureSize - padding * 2) / height
    );
    context.translate(
      (textureSize - width * scale) / 2 - minX * scale,
      (textureSize - height * scale) / 2 - minY * scale
    );
    context.scale(scale, scale);
    context.fillStyle = token('--color-on-surface');
    const defaultFillRule = svg.getAttribute('fill-rule') === 'evenodd' ? 'evenodd' : 'nonzero';
    for (const path of document.querySelectorAll('path')) {
      const data = path.getAttribute('d');
      if (!data) continue;
      context.globalAlpha = Number(path.getAttribute('opacity') ?? 1);
      const fillRule = path.getAttribute('fill-rule') === 'evenodd' ? 'evenodd' : defaultFillRule;
      context.fill(new Path2D(data), fillRule);
    }

    return makeIconSprite(canvas, radius, 0.94);
  }

  function makeProjectSprite(radius: number): Sprite {
    const canvas = window.document.createElement('canvas');
    const textureSize = 256;
    const padding = 32;
    canvas.width = textureSize;
    canvas.height = textureSize;
    const context = canvas.getContext('2d');
    if (!context) return new Sprite();
    const scale = (textureSize - padding * 2) / 24;
    context.translate(padding, padding);
    context.scale(scale, scale);
    context.strokeStyle = token('--color-surface-sunken');
    context.lineWidth = 2;
    context.lineCap = 'round';
    context.lineJoin = 'round';
    context.globalAlpha = 0.92;

    context.stroke(
      new Path2D(
        'M9 20H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H20a2 2 0 0 1 2 2v5'
      )
    );
    context.beginPath();
    context.arc(13, 12, 2, 0, Math.PI * 2);
    context.stroke();
    context.stroke(new Path2D('M18 19c-2.8 0-5-2.2-5-5v8'));
    context.beginPath();
    context.arc(20, 19, 2, 0, Math.PI * 2);
    context.stroke();

    return makeIconSprite(canvas, radius, 0.82);
  }

  function makeToolSprite(radius: number): Sprite {
    const canvas = window.document.createElement('canvas');
    const textureSize = 256;
    const padding = 32;
    canvas.width = textureSize;
    canvas.height = textureSize;
    const context = canvas.getContext('2d');
    if (!context) return new Sprite();
    const scale = (textureSize - padding * 2) / 24;
    context.translate(padding, padding);
    context.scale(scale, scale);
    context.strokeStyle = token('--color-surface-sunken');
    context.lineWidth = 2;
    context.lineCap = 'round';
    context.lineJoin = 'round';
    context.globalAlpha = 0.92;
    context.stroke(
      new Path2D(
        'M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z'
      )
    );

    return makeIconSprite(canvas, radius, 0.82);
  }

  function makeIconSprite(canvas: HTMLCanvasElement, radius: number, size: number): Sprite {
    const texture = new CanvasTexture(canvas);
    texture.colorSpace = SRGBColorSpace;
    texture.needsUpdate = true;
    const material = new SpriteMaterial({
      map: texture,
      transparent: true,
      opacity: 0.96,
      depthTest: false,
      depthWrite: false
    });
    const sprite = new Sprite(material);
    sprite.scale.set(radius * size, radius * size, 1);
    sprite.position.z = radius * 0.28;
    sprite.renderOrder = 19;
    return sprite;
  }

  function makeNodeObject(node: SceneNode): Object3D {
    const group = new Group();
    const color = kindColor(node.kind);
    const bodyMaterial = new MeshStandardMaterial({
      color,
      emissive: new Color(color),
      emissiveIntensity: anchor(node.kind) ? 0.12 : 0.06,
      flatShading: true,
      metalness: node.kind === 'model' ? 0.14 : 0.04,
      roughness: 0.72,
      transparent: true,
      opacity: 1
    });
    const body = makeBody(node.kind, node.radius, bodyMaterial);
    group.add(body);
    const iconSprite =
      node.kind === 'model'
        ? makeProviderSprite(node.provider, node.radius)
        : node.kind === 'project'
          ? makeProjectSprite(node.radius)
          : node.kind === 'tool'
            ? makeToolSprite(node.radius)
            : null;
    if (iconSprite) group.add(iconSprite);

    const haloMaterial = new MeshBasicMaterial({
      color: token('--color-on-surface'),
      transparent: true,
      opacity: 0
    });
    const halo = new Group();
    const firstOrbit = new Mesh(
      new TorusGeometry(node.radius * 1.32, Math.max(0.12, node.radius * 0.035), 7, 36),
      haloMaterial
    );
    firstOrbit.rotation.set(0.58, 0.2, 0);
    halo.add(firstOrbit);
    const secondOrbit = new Mesh(
      new TorusGeometry(node.radius * 1.42, Math.max(0.1, node.radius * 0.026), 7, 36),
      haloMaterial
    );
    secondOrbit.rotation.set(0.12, 0.88, 0.24);
    halo.add(secondOrbit);
    halo.visible = false;
    group.add(halo);

    const label = makeTextSprite(
      compactLabel(node),
      token('--color-on-surface'),
      1,
      true
    );
    const direction = labelDirection(node);
    label.position.set(
      direction * (node.radius + label.scale.x / 2 + 3),
      (unit(node.id, 43) - 0.5) * Math.max(7, node.radius * 0.9),
      0
    );
    label.visible = persistentLabelIds.has(node.id);
    group.add(label);

    const labelMaterial = label.material as SpriteMaterial;
    nodeVisuals.set(node.id, {
      group,
      bodyMaterial,
      halo,
      haloMaterial,
      label,
      labelMaterial,
      iconMaterial: iconSprite ? (iconSprite.material as SpriteMaterial) : null
    });
    return group;
  }

  function disposeObject(object: Object3D) {
    object.traverse((child) => {
      const mesh = child as Mesh;
      mesh.geometry?.dispose();
      const materials = mesh.material
        ? Array.isArray(mesh.material)
          ? mesh.material
          : [mesh.material]
        : [];
      for (const material of materials) {
        const textured = material as Material & { map?: { dispose: () => void } };
        textured.map?.dispose();
        material.dispose();
      }
    });
  }

  function rebuildGraph() {
    if (!graph) return;
    for (const visual of nodeVisuals.values()) disposeObject(visual.group);
    nodeVisuals.clear();
    persistentLabelIds.clear();
    removeGuides();

    const nodeValues = nodes.map((node) => metricValue(node));
    const radiusScale = scaleSqrt()
      .domain([0, Math.max(1, ...nodeValues)])
      .range([4.5, 14.5]);
    sceneNodes = nodes.map((node) => {
      const radius = Math.max(anchor(node.kind) ? 7 : 4.5, radiusScale(metricValue(node)));
      return {
        ...node,
        radius,
        value: metricValue(node),
        x: xTarget(node.kind) + (unit(node.id, 11) - 0.5) * 38,
        y: yTarget(node),
        z: zTarget(node.kind) + (unit(node.id, 17) - 0.5) * 42
      };
    });
    const rankedAnchors = [...sceneNodes]
      .filter((node) => anchor(node.kind))
      .sort((left, right) => right.value - left.value || left.label.localeCompare(right.label));
    for (const node of rankedAnchors) {
      const sameKindRank = rankedAnchors.filter((candidate) => candidate.kind === node.kind).indexOf(node);
      const limit = node.kind === 'tool' ? 5 : sceneNodes.length > 32 ? 5 : 7;
      if (sameKindRank < limit) persistentLabelIds.add(node.id);
    }
    sceneLinks = edges.map((edge) => ({
      ...edge,
      source: edge.source,
      target: edge.target,
      value: metricValue(edge)
    }));

    graph
      .warmupTicks(reduceMotion ? 260 : 90)
      .cooldownTicks(reduceMotion ? 0 : 260)
      .cooldownTime(reduceMotion ? 0 : 5_500)
      .d3AlphaDecay(0.035)
      .d3VelocityDecay(0.3)
      .graphData({ nodes: sceneNodes, links: sceneLinks });

    graph.d3Force('x', forceX<SceneNode>((node) => xTarget(node.kind)).strength(0.18));
    graph.d3Force('y', forceY<SceneNode>((node) => yTarget(node)).strength(0.055));
    graph.d3Force('z', zForce());
    const charge = graph.d3Force('charge') as unknown as {
      strength: (value: (node: SceneNode) => number) => void;
    };
    charge?.strength((node) => (anchor(node.kind) ? -225 : -125));
    const linkForce = graph.d3Force('link') as unknown as ForceLink<SceneNode, SceneLink>;
    linkForce
      ?.distance((link) => (link.relation.includes('core') || link.relation.includes('mcp') ? 105 : 82))
      .strength((link) => (link.relation === 'project_model' ? 0.26 : 0.38));

    addGuides();
    needsVisualSync = true;
    const shouldRestore = Boolean(camera) && !skipCameraRestore;
    skipCameraRestore = false;
    needsInitialFit = !shouldRestore;
    if (camera && shouldRestore) restoreCamera(camera);
  }

  function removeGuides() {
    if (!guideGroup || !graph) return;
    graph.scene().remove(guideGroup);
    disposeObject(guideGroup);
    guideGroup = null;
  }

  function addGuides() {
    if (!graph) return;
    const group = new Group();
    const grid = new GridHelper(
      520,
      10,
      new Color(token('--color-border')),
      new Color(token('--color-border-soft'))
    );
    grid.position.y = -155;
    const gridMaterial = grid.material as Material;
    gridMaterial.transparent = true;
    gridMaterial.opacity = 0.2;
    group.add(grid);

    const kinds: GraphNodeKind[] = ['project', 'tool', 'model', 'core_tool', 'mcp_server'];
    for (const kind of kinds) {
      const total = sceneNodes.filter((node) => node.kind === kind).length;
      if (!total) continue;
      const label = makeTextSprite(`${kindLabel(kind)} · ${count(total)}`, token('--color-muted-2'), 0.72);
      label.position.set(xTarget(kind), 148, zTarget(kind));
      label.scale.multiplyScalar(0.88);
      group.add(label);
    }
    guideGroup = group;
    graph.scene().add(group);
  }

  function endpointId(endpoint: string | SceneNode | number | undefined): string {
    if (typeof endpoint === 'object' && endpoint) return endpoint.id;
    return String(endpoint ?? '');
  }

  function focusedNeighborhood(): Set<string> {
    const focusId = selectedId ?? hoveredId;
    const neighborhood = new Set<string>();
    if (!focusId) return neighborhood;
    neighborhood.add(focusId);
    for (const link of sceneLinks) {
      const source = endpointId(link.source);
      const target = endpointId(link.target);
      if (source === focusId) neighborhood.add(target);
      if (target === focusId) neighborhood.add(source);
    }
    return neighborhood;
  }

  function isConnected(link: SceneLink, focusId = selectedId ?? hoveredId): boolean {
    if (!focusId) return false;
    return endpointId(link.source) === focusId || endpointId(link.target) === focusId;
  }

  function linkColor(link: SceneLink): string {
    const focusId = selectedId ?? hoveredId;
    if (!focusId) return subduedRelationColor(link.relation);
    return isConnected(link, focusId)
      ? token('--color-on-surface')
      : inactiveRelationColor(link.relation);
  }

  function linkWidth(link: SceneLink): number {
    const values = sceneLinks.map((candidate) => candidate.value);
    const scale = scaleSqrt()
      .domain([0, Math.max(1, ...values)])
      .range([0.3, 2.65]);
    const base = scale(link.value);
    if (!selectedId && !hoveredId) return base;
    return isConnected(link) ? base * 1.65 : 0.12;
  }

  function linkCurvature(link: SceneLink): number {
    return (unit(link.id, 29) - 0.5) * 0.24;
  }

  function updateVisualState() {
    if (!graph || !sceneNodes.length) return;
    const focusId = selectedId ?? hoveredId;
    const neighborhood = focusedNeighborhood();
    for (const node of sceneNodes) {
      const visual = nodeVisuals.get(node.id);
      if (!visual) continue;
      const selected = node.id === selectedId;
      const hovered = node.id === hoveredId;
      const related = !focusId || neighborhood.has(node.id);
      visual.group.scale.setScalar(selected ? 1.24 : hovered ? 1.13 : 1);
      visual.bodyMaterial.opacity = related ? 1 : 0.13;
      visual.bodyMaterial.emissiveIntensity = selected ? 0.48 : hovered ? 0.32 : anchor(node.kind) ? 0.18 : 0.08;
      visual.halo.visible = selected || hovered;
      visual.haloMaterial.opacity = selected ? 0.82 : hovered ? 0.56 : 0;
      visual.label.visible = focusId ? related : persistentLabelIds.has(node.id);
      visual.labelMaterial.opacity = related ? 1 : 0.12;
      if (visual.iconMaterial) visual.iconMaterial.opacity = related ? 0.96 : 0.12;
    }
    graph.linkColor((link) => linkColor(link)).linkWidth((link) => linkWidth(link)).refresh();
  }

  function template(text: string, values: Record<string, string>): string {
    return Object.entries(values).reduce(
      (output, [key, value]) => output.split(`{${key}}`).join(value),
      text
    );
  }

  function nodeAria(node: GraphNode): string {
    return template(copy.node_aria, {
      kind: kindLabel(node.kind),
      label: node.label,
      calls: count(node.stats.calls)
    });
  }

  function nodeTooltip(node: SceneNode): HTMLElement {
    const tooltip = document.createElement('div');
    tooltip.className = 'relationship-tooltip';
    const title = document.createElement('strong');
    title.textContent = node.label;
    const detail = document.createElement('span');
    detail.textContent = nodeAria(node);
    const value = document.createElement('span');
    value.textContent = metric === 'spend' ? node.stats.cost : count(node.value);
    tooltip.append(title, detail, value);
    return tooltip;
  }

  function linkTooltip(link: SceneLink): HTMLElement {
    const tooltip = document.createElement('div');
    tooltip.className = 'relationship-tooltip';
    const title = document.createElement('strong');
    title.textContent = relationLabel(link.relation);
    const value = document.createElement('span');
    value.textContent = metric === 'spend' ? link.stats.cost : count(link.value);
    tooltip.append(title, value);
    return tooltip;
  }

  function persistCamera() {
    if (!graph || !controls) return;
    const position = graph.cameraPosition();
    saveCamera({
      x: position.x,
      y: position.y,
      z: position.z,
      targetX: controls.target.x,
      targetY: controls.target.y,
      targetZ: controls.target.z
    });
  }

  function restoreCamera(value: GraphCamera) {
    graph?.cameraPosition(
      { x: value.x, y: value.y, z: value.z },
      { x: value.targetX, y: value.targetY, z: value.targetZ },
      0
    );
  }

  function handleEngineFrame() {
    if (needsVisualSync) {
      needsVisualSync = false;
      updateVisualState();
    }
    if (!needsInitialFit) return;
    needsInitialFit = false;
    frameGraph(false, 0);
  }

  function frameGraph(includeAll: boolean, duration = reduceMotion ? 0 : 620) {
    if (!graph || sceneNodes.length === 0) return;
    const frameNodes = includeAll
      ? sceneNodes
      : [...sceneNodes]
          .sort((left, right) => right.value - left.value || left.label.localeCompare(right.label))
          .slice(0, Math.min(24, sceneNodes.length));
    const xs = frameNodes.map((node) => node.x);
    const ys = frameNodes.map((node) => node.y);
    const zs = frameNodes.map((node) => node.z);
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const minY = Math.min(...ys);
    const maxY = Math.max(...ys);
    const minZ = Math.min(...zs);
    const maxZ = Math.max(...zs);
    const center = {
      x: (minX + maxX) / 2,
      y: (minY + maxY) / 2,
      z: (minZ + maxZ) / 2
    };
    const spanX = Math.max(80, maxX - minX + 52);
    const spanY = Math.max(80, maxY - minY + 52);
    const spanZ = Math.max(60, maxZ - minZ + 36);
    const aspect = Math.max(0.5, graph.width() / graph.height());
    const cameraWithFov = graph.camera() as { fov?: number };
    const halfFov = ((cameraWithFov.fov ?? 60) * Math.PI) / 360;
    const distanceX = spanX / (2 * Math.tan(halfFov) * aspect);
    const distanceY = spanY / (2 * Math.tan(halfFov));
    const densityMargin = includeAll && frameNodes.length > 40 ? 1.42 : frameNodes.length > 24 ? 1.3 : 1.16;
    const distance = Math.max(155, distanceX, distanceY, spanZ * 1.25) * densityMargin;
    const direction = { x: 0.16, y: 0.1, z: 0.982 };
    const position = {
      x: center.x + direction.x * distance,
      y: center.y + direction.y * distance,
      z: center.z + direction.z * distance
    };
    graph.cameraPosition(position, center, duration);
    window.setTimeout(persistCamera, duration + 40);
  }

  export function fit() {
    frameGraph(true);
  }

  export function focus(id: string) {
    if (!graph) return;
    const node = sceneNodes.find((candidate) => candidate.id === id);
    if (!node) return;
    const current = graph.cameraPosition();
    const dx = current.x - node.x;
    const dy = current.y - node.y;
    const dz = current.z - node.z;
    const length = Math.hypot(dx, dy, dz) || 1;
    const distance = Math.max(68, node.radius * 7.5);
    const next = {
      x: node.x + (dx / length) * distance,
      y: node.y + (dy / length) * distance,
      z: node.z + (dz / length) * distance
    };
    const duration = reduceMotion ? 0 : 520;
    graph.cameraPosition(next, { x: node.x, y: node.y, z: node.z }, duration);
    window.setTimeout(persistCamera, duration + 40);
  }

  export function reset() {
    skipCameraRestore = true;
    saveCamera(null);
    layoutRevision += 1;
  }

  function handleKeyboard(event: KeyboardEvent) {
    if (!nodes.length) return;
    const currentIndex = selectedId ? nodes.findIndex((node) => node.id === selectedId) : -1;
    let nextIndex = currentIndex;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
      nextIndex = (currentIndex + 1 + nodes.length) % nodes.length;
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
      nextIndex = (currentIndex - 1 + nodes.length) % nodes.length;
    } else if (event.key === 'Home') {
      nextIndex = 0;
    } else if (event.key === 'End') {
      nextIndex = nodes.length - 1;
    } else if ((event.key === 'Enter' || event.key === ' ') && selectedId) {
      event.preventDefault();
      focus(selectedId);
      return;
    } else {
      return;
    }
    event.preventDefault();
    const node = nodes[nextIndex];
    selectNode(node.id);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions -->
<div
  class="graph-canvas"
  class:has-selection={Boolean(selectedId)}
  bind:this={container}
  role="application"
  tabindex="0"
  aria-label={`${copy.canvas_aria}. ${copy.keyboard_hint}`}
  onkeydown={handleKeyboard}
>
  <div class="canvas-readout" aria-hidden="true">
    <span class="mode-badge">{copy.mode_3d}</span>
    <span>{copy.interaction_hint}</span>
  </div>
  <div class="sr-only" aria-live="polite">
    {selectedAriaNode ? nodeAria(selectedAriaNode) : copy.keyboard_hint}
  </div>
</div>

<style>
  .graph-canvas {
    position: relative;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 540px;
    height: 100%;
    background: var(--color-surface-sunken);
    overflow: hidden;
    outline: none;
  }

  .graph-canvas:focus-visible {
    outline: 2px solid var(--color-primary);
    outline-offset: -2px;
  }

  .canvas-readout {
    position: absolute;
    z-index: 2;
    right: var(--space-lg);
    bottom: var(--space-lg);
    left: var(--space-lg);
    display: flex;
    align-items: center;
    gap: var(--space-md);
    color: var(--color-muted-2);
    font-size: 10px;
    pointer-events: none;
  }

  .mode-badge {
    flex: 0 0 auto;
    padding: 3px 6px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--color-surface-sunken) 88%, transparent);
    color: var(--color-cyan);
    font-family: var(--font-mono);
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  :global(.graph-canvas .scene-container) {
    position: absolute !important;
    inset: 0;
  }

  :global(.graph-canvas canvas) {
    display: block;
  }

  :global(.graph-canvas .graph-tooltip) {
    padding: 0 !important;
    border: 0 !important;
    background: transparent !important;
    color: inherit !important;
  }

  :global(.relationship-tooltip) {
    min-width: 140px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: var(--space-md);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-neutral);
    color: var(--color-on-surface);
    box-shadow: var(--elevation-popover);
    font-family: var(--font-ui);
    text-align: left;
  }

  :global(.relationship-tooltip strong) {
    font-size: 12px;
  }

  :global(.relationship-tooltip span) {
    color: var(--color-muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  @media (max-width: 820px) {
    .graph-canvas {
      flex: 0 0 auto;
      height: 520px;
      min-height: 520px;
    }

    .canvas-readout > span:last-child {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .graph-canvas {
      scroll-behavior: auto;
    }
  }
</style>
