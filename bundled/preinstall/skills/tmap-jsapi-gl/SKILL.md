---
name: "tmap-jsapi-gl"
description: "Build map features with the Tencent Maps JavaScript API GL: markers, overlays, layers, events, services, geometry, 3D models, visualization. Use when writing or debugging Tencent Maps code."
description_zh: "使用腾讯地图 JavaScript API GL 开发地图功能：标记、覆盖物、图层、事件、服务、几何计算和数据可视化"
version: "1.0.0"
source: agentz
---

# TMap JSAPI GL Skill

Helps users build map features with the Tencent Maps JavaScript API GL, including basic map features and data visualization.

The reference documents and demos are in Chinese (the language of the Tencent Maps API); their file names are Chinese too. English glosses are given below.

## Directory structure

### API documents

- **JS API reference**: `references/jsapigl/docs/` (21 md files)
  - 概述.md - API overview and index
  - 地图.md - core map class and configuration
  - 点标记.md - marker API
  - 矢量图形.md - polylines, polygons, circles, rectangles, ellipses and other vector shapes
  - 文本标记.md - text label API
  - DOM 覆盖物.md - custom DOM overlays
  - 信息窗体.md - info window API
  - 点聚合.md - marker clustering
  - 控件.md - map controls
  - 自定义图层.md - custom raster/vector layers
  - 事件.md - map event system
  - 基础类.md - base classes such as LatLng and Point
  - 室内图.md - indoor maps
  - 附加库：地图工具.md - geometry editor, measurement tools
  - 附加库：几何计算库.md - distance and area calculation
  - 附加库：服务类库.md - place search, route planning and other services
  - 附加库：地图视角附加库.md - observer view
  - 附加库：模型库.md - GLTF/3DTiles models
  - 附加库：天气图层.md - weather layers
  - 附加库：矢量数据图层.md - GeoJSON/MVT layers
  - 环境检测.md - browser environment detection

- **Visualization reference**: `references/visualization/docs/` (15 md files)
  - 参考手册.md - visualization API overview
  - 弧线图.md - 3D arc / flow map
  - 散点图.md - 3D scatter plot
  - 热力图.md - classic heat map
  - 蜂窝热力图.md - hexagon-aggregated heat map
  - 网格热力图.md - grid-aggregated heat map
  - 轨迹图.md - trajectory display
  - 区域图.md - area outline map
  - 管道图.md - 3D pipe map
  - 辐射圈.md - radiating circle effect
  - 围墙面.md - wall effect
  - 水晶体.md - 3D crystal effect
  - 行政区划.md - administrative divisions
  - 事件.md - visualization event system
  - 基础类.md - visualization base classes

### Example code

- **JS API demos**: `references/jsapigl/demos/` (129 html files), grouped by feature: map operations, markers, text labels, clustering, polylines, polygons, controls, info windows, services, custom map styles, geometry calculation, models, tools, custom overlays, city tours and more.
- **Visualization demos**: `references/visualization/demos/` (44 html files), grouped by layer type: arc, scatter, heat map, trajectory, hexagon, area, crystal and more.

Demo file names follow `<category>_<example>.html`.

## Workflow

### 1. Understand the request

When the user asks about the Tencent Maps API:

- Decide whether it is basic map functionality or visualization.
- Identify the specific class or feature needed.

### 2. Look up the API documents

Search `references/jsapigl/docs/` or `references/visualization/docs/` (for example for "点标记" or "热力图"), then read the class description, config parameters and methods.

### 3. Find example code

Look in the matching demos directory:

- JS API demos: `references/jsapigl/demos/`
- Visualization demos: `references/visualization/demos/`

### 4. Provide the solution

Based on the documents and demos, give the user:

- API explanation
- Code example
- Caveats and best practices

## Usage examples

**User question**: "How do I add a marker to the map?"

1. Read `references/jsapigl/docs/点标记.md` to learn the MultiMarker API.
2. Look at the marker demos in `references/jsapigl/demos/`.
3. Provide complete code and an explanation.

**User question**: "How do I draw a heat map?"

1. Read `references/visualization/docs/热力图.md` to learn the Heat API.
2. Look at the heat map demos in `references/visualization/demos/`.
3. Explain the data format and options.

## Quick start templates

Basic map initialization:

```html
<!DOCTYPE html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>Tencent Maps example</title>
    <script src="https://map.qq.com/api/gljs?v=3&key=[YOUR_KEY]"></script>
    <!-- For visualization, add: &libraries=visualization -->
  </head>
  <body>
    <div id="map" style="width:100%;height:500px;"></div>
    <script>
      var map = new TMap.Map('map', {
        zoom: 12,
        center: new TMap.LatLng(39.984104, 116.307503),
      });
    </script>
  </body>
</html>
```

Visualization layer example (heat map):

```javascript
// Load the visualization library
// <script src="https://map.qq.com/api/gljs?v=1.beta&libraries=visualization&key=[YOUR_KEY]"></script>

var heat = new TMap.visualization.Heat({
  radius: 50,
  height: 100,
  gradientColor: {
    0: '#13B06A',
    0.4: '#13B06A',
    0.8: '#E9AB1D',
    0.9: '#E9AB1D',
    1: '#E05649',
  },
}).addTo(map);

heat.setData([
  { lat: 39.984104, lng: 116.307503, count: 100 },
  { lat: 39.984504, lng: 116.307803, count: 80 },
]);
```

## Notes

### JS API GL

1. **API key**: a Tencent Maps key is required.
2. **Version**: this is the GL version, with 3D maps and WebGL rendering.
3. **Browser support**: modern browsers; IE11+ needs a polyfill.
4. **Coordinate system**: gcj02.
5. **Map creation (important)**: the map container must have a fixed width and height, especially in flex layouts.
6. **API usage (important)**: only use interfaces, properties and events that appear in the documents; never invent API.
7. **API parameters (important)**: pass parameters in exactly the format the API documents describe. If unsure, check the matching demo, including its data format.
8. **Additional libraries**: add the `libraries` parameter to the API loading URL.

| Library | libraries value | Namespace | Description |
| --- | --- | --- | --- |
| Map tools | `tools` | `TMap.tools` | Geometry editor, measurement tools |
| Geometry | `geometry` | `TMap.geometry` | Distance/area calculation, spatial relations |
| Service | `service` | `TMap.service` | Place search, route planning, administrative divisions |
| View | `view` | `TMap` (extension methods) | Observer-view map control |
| Model | `model` | `TMap.model` | GLTF/3DTiles/3DMarker models |
| Weather | `weather` | `TMap.weather` | Cloud, temperature and other weather layers |
| Vector | `vector` | `TMap.vector` | GeoJSON/MVT vector layers |
| Visualization | `visualization` | `TMap.visualization` | Visualization API |

Example:

```html
<!-- Load several additional libraries -->
<script src="https://map.qq.com/api/gljs?v=1&libraries=tools,geometry,service,model&key=[YOUR_KEY]"></script>
```

### Visualization API

1. **Data format**: visualization layers need input data in a specific format.
2. **Performance**: watch performance with large data sets.
3. **Layers**: visualization layers can be assigned display levels.
4. **Events**: click, hover and other interaction events are supported.
5. **API usage (important)**: only use interfaces, properties and events that appear in the documents; never invent API.
6. **API parameters (important)**: pass parameters in exactly the documented format; if unsure, check the matching demo, including its data format.

## Best practices

1. **Modular loading**: load the API modularly, only what you need.
2. **Error handling**: handle map loading failures.
3. **Memory management**: destroy layers and overlays you no longer need.
4. **Performance**: use clustering or down-sampling for large data sets.
