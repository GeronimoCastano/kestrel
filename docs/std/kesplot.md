# std.kesplot

Plotting with a matplotlib-shaped API: a `Figure` holds panels, an `Axes` draws,
and styles are values.

```kestrel
import std.kesplot

let figure := Figure()
figure.plot([1.0, 2.0, 3.0], [2.0, 4.0, 3.0], label = "trend")
figure.xLabel("step")
figure.save("plot.svg", dpi = 144)?
```

Every member of the module follows.


### Types

- `Style`: A named set of plotting defaults.
- `Figure`: A drawing surface: axes, panels, size and a save path.

### Style

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Style.copy() : Style` | A value with the same contents. |
| `isSerif` | `Style.isSerif() : Bool` | True when the font is serif. |
| `isMono` | `Style.isMono() : Bool` | True when the series only moves one way. |
| `withFontSize` | `Style.withFontSize(size : double) : Style` | A copy with a new font size. |
| `withFontFamily` | `Style.withFontFamily(family : String, kind : String) : Style` | A copy with a new font family. |
| `withColors` | `Style.withColors(colors : List[String]) : Style` | A copy with new colours. |
| `withFigureSize` | `Style.withFigureSize(width : double, height : double) : Style` | A copy with a new figure size. |

### Functions

| Member | Signature | What it does |
|---|---|---|
| `componentNames` | `Style.componentNames() : List[String]` | The names of the tensor components. |
| `names` | `Style.names() : List[String]` | The column names. |
| `isComponent` | `Style.isComponent(name : String) : Bool` | True for a component value. |
| `applyComponent` | `Style.applyComponent(base : Style, component : String) : Style` | Maps a function over one component. |
| `normalizeName` | `Style.normalizeName(name : String) : String` | The canonical form of a column name. |
| `byName` | `Style.byName(name : String) : Style or Error` | Groups the frame by a column name. |
| `isKnown` | `Style.isKnown(name : String) : Bool` | True when the value is not missing. |
| `named` | `Style.named(name : String) : Style` | A copy with a column name. |
| `standard` | `Style.standard() : Style` | The standard plotting style. |
| `classic` | `Style.classic() : Style` | The classic Matplotlib style. |
| `ggplot` | `Style.ggplot() : Style` | The ggplot plotting style. |
| `seaborn` | `Style.seaborn() : Style` | The seaborn plotting style. |
| `seabornWhitegrid` | `Style.seabornWhitegrid() : Style` | The seaborn white-grid style. |
| `bmh` | `Style.bmh() : Style` | The bayesian-methods-for-hackers style. |
| `dark` | `Style.dark() : Style` | The dark plotting style. |
| `fivethirtyeight` | `Style.fivethirtyeight() : Style` | The FiveThirtyEight plotting style. |
| `grayscale` | `Style.grayscale() : Style` | The grayscale plotting style. |
| `solarize` | `Style.solarize() : Style` | The Solarize Light plotting style. |
| `minimal` | `Style.minimal() : Style` | The minimal plotting style. |
| `science` | `Style.science() : Style` | The Science plotting style. |
| `ieee` | `Style.ieee() : Style` | The IEEE plotting style. |
| `nature` | `Style.nature() : Style` | The Nature-journal plotting style. |
| `scienceIeee` | `Style.scienceIeee() : Style` | The Science IEEE style. |
| `scienceNature` | `Style.scienceNature() : Style` | The Science Nature style. |
| `scienceGrid` | `Style.scienceGrid() : Style` | The Science grid style. |
| `scienceScatter` | `Style.scienceScatter() : Style` | The Science scatter style. |
| `scienceBright` | `Style.scienceBright() : Style` | The Science bright style. |
| `scienceVibrant` | `Style.scienceVibrant() : Style` | The Science vibrant style. |
| `scienceMuted` | `Style.scienceMuted() : Style` | The Science muted style. |
| `scienceHighVis` | `Style.scienceHighVis() : Style` | The Science high-visibility style. |
| `scienceHighContrast` | `Style.scienceHighContrast() : Style` | The Science high-contrast style. |
| `scienceLight` | `Style.scienceLight() : Style` | The Science light style. |
| `scienceRetro` | `Style.scienceRetro() : Style` | The Science retro style. |
| `notebook` | `Style.notebook() : Style` | Whether the output targets a notebook. |
| `version` | `std.kesplot.version() : String` | The library version. |
| `backendReport` | `std.kesplot.backendReport() : String with Effects` | What the plotting backend can draw here. |

### Svg

| Member | Signature | What it does |
|---|---|---|
| `add` | `Svg.add(text : String) : void` | Adds an element to the SVG. |
| `text` | `Svg.text() : String` | The SVG element for text. |

### Line2D

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Line2D.copy() : Line2D` | A value with the same contents. |

### Points

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Points.copy() : Points` | A value with the same contents. |

### Bars

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Bars.copy() : Bars` | A value with the same contents. |

### Band

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Band.copy() : Band` | A value with the same contents. |

### ErrorBars

| Member | Signature | What it does |
|---|---|---|
| `copy` | `ErrorBars.copy() : ErrorBars` | A value with the same contents. |

### BoxPlot

| Member | Signature | What it does |
|---|---|---|
| `copy` | `BoxPlot.copy() : BoxPlot` | A value with the same contents. |

### Guide

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Guide.copy() : Guide` | A value with the same contents. |

### Contour

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Contour.copy() : Contour` | A value with the same contents. |

### Heatmap

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Heatmap.copy() : Heatmap` | A value with the same contents. |

### Pie

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Pie.copy() : Pie` | A value with the same contents. |

### Annotation

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Annotation.copy() : Annotation` | A value with the same contents. |

### TextMark

| Member | Signature | What it does |
|---|---|---|
| `copy` | `TextMark.copy() : TextMark` | A value with the same contents. |

### LegendItem

| Member | Signature | What it does |
|---|---|---|
| `copy` | `LegendItem.copy() : LegendItem` | A value with the same contents. |

### Colorbar

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Colorbar.copy() : Colorbar` | A value with the same contents. |

### Bounds

| Member | Signature | What it does |
|---|---|---|
| `include` | `Bounds.include(x : double) : void` | Whether the index is written. |

### Transform

| Member | Signature | What it does |
|---|---|---|
| `px` | `Transform.px(x : double) : double` | See the signature. |
| `py` | `Transform.py(y : double) : double` | See the signature. |

### Axes

| Member | Signature | What it does |
|---|---|---|
| `copy` | `Axes.copy() : Axes` | A value with the same contents. |
| `addLine` | `Axes.addLine(line : Line2D) : void` | Adds a line to the axes. |
| `addPoints` | `Axes.addPoints(points : Points) : void` | Adds point markers to the axes. |
| `addBars` | `Axes.addBars(bars : Bars) : void` | Adds bar geometry to the axes. |
| `addBand` | `Axes.addBand(band : Band) : void` | Draws a filled band between two y series. |
| `addErrorBars` | `Axes.addErrorBars(errors : ErrorBars) : void` | Adds error bars to the axes. |
| `addBox` | `Axes.addBox(box : BoxPlot) : void` | Adds a box geometry to the axes. |
| `addGuide` | `Axes.addGuide(guide : Guide) : void` | Adds a guide line to the legend output. |
| `addContour` | `Axes.addContour(contour : Contour) : void` | Adds contour lines to the axes. |
| `addHeatmap` | `Axes.addHeatmap(map : Heatmap) : void` | Adds a heat map to the axes. |
| `addPie` | `Axes.addPie(pie : Pie) : void` | Adds a pie chart to the axes. |
| `addText` | `Axes.addText(text : TextMark) : void` | Adds text at a position. |
| `addAnnotation` | `Axes.addAnnotation(annotation : Annotation) : void` | Adds a text annotation to the axes. |
| `addLegendItem` | `Axes.addLegendItem(item : LegendItem) : void` | Registers one legend entry. |
| `categories` | `Axes.categories(labels : List[String]) : List[double]` | The distinct categories of a series. |
| `categoriesY` | `Axes.categoriesY(labels : List[String]) : List[double]` | The categories along the y axis. |

### Figure

| Member | Signature | What it does |
|---|---|---|
| `panelCount` | `Figure.panelCount() : Int` | How many panels. |
| `panelRows` | `Figure.panelRows() : Int` | The rows of the panel grid. |
| `panelColumns` | `Figure.panelColumns() : Int` | The columns of the panel grid. |
| `currentPanel` | `Figure.currentPanel() : Int` | The panel that is being drawn. |
| `subplots` | `Figure.subplots(rows : Int, cols : Int) : void` | A figure with a grid of panels. |
| `panel` | `Figure.panel(index : Int) : void` | A subplot panel. |
| `nextPanel` | `Figure.nextPanel() : void` | The next panel of the figure. |
| `figureTitleText` | `Figure.figureTitleText(text : String, size : double) : void` | The figure title as text. |
| `size` | `Figure.size(width : double, height : double) : void` | How many values. |
| `useStyle` | `Figure.useStyle(next : Style) : void` | Applies a style to the figure. |
| `useStyleName` | `Figure.useStyleName(name : String) : void or Error` | Applies a named style. |
| `useTheme` | `Figure.useTheme(name : String) : void or Error` | Applies a theme. |
| `styleName` | `Figure.styleName() : String` | The name of the style. |
| `margins` | `Figure.margins(left : double, right : double, bottom : double, top : double) : void` | The figure margins. |
| `spacing` | `Figure.spacing(wspace : double, hspace : double) : void` | The spacing between things. |
| `tightLayout` | `Figure.tightLayout() : void` | Tightens the layout. |
| `title` | `Figure.title(text : String) : void` | Sets the axes title. |
| `xLabel` | `Figure.xLabel(text : String) : void` | The x axis label. |
| `yLabel` | `Figure.yLabel(text : String) : void` | The y axis label. |
| `xScale` | `Figure.xScale(scale : String) : void` | The scale of the x axis. |
| `yScale` | `Figure.yScale(scale : String) : void` | The scale of the y axis. |
| `xLimits` | `Figure.xLimits(low : double, high : double) : void` | The x axis limits. |
| `yLimits` | `Figure.yLimits(low : double, high : double) : void` | The y axis limits. |
| `xAuto` | `Figure.xAuto() : void` | Whether the x limits are chosen automatically. |
| `invertX` | `Figure.invertX() : void` | Reverses the x axis. |
| `invertY` | `Figure.invertY() : void` | Reverses the y axis. |
| `yAuto` | `Figure.yAuto() : void` | Whether the y limits are chosen automatically. |
| `xTickRotation` | `Figure.xTickRotation(degrees : double) : void` | The rotation of the x tick labels. |
| `aspect` | `Figure.aspect(value : String) : void` | Sets the ratio of the axes. |
| `xTicks` | `Figure.xTicks(positions : List[double]) : void` | The x tick positions. |
| `xTicks` | `Figure.xTicks(positions : List[double], labels : List[String]) : void` | The x tick positions. |
| `xTicks` | `Figure.xTicks(labels : List[String]) : void` | The x tick positions. |
| `yTicks` | `Figure.yTicks(positions : List[double]) : void` | The y tick positions. |
| `yTicks` | `Figure.yTicks(positions : List[double], labels : List[String]) : void` | The y tick positions. |
| `yTicks` | `Figure.yTicks(labels : List[String]) : void` | The y tick positions. |
| `grid` | `Figure.grid(enabled : Bool) : void` | Turns the grid on or off. |
| `grid` | `Figure.grid(enabled : Bool, axis : String) : void` | Turns the grid on or off. |
| `minorGrid` | `Figure.minorGrid(enabled : Bool) : void` | Minor grid lines. |
| `hideSpines` | `Figure.hideSpines() : void` | Hides the axes spines. |
| `spines` | `Figure.spines(top : Bool, right : Bool, left : Bool, bottom : Bool) : void` | The axes spines. |
| `legend` | `Figure.legend() : void` | The legend of the figure. |
| `legend` | `Figure.legend(position : String) : void` | The legend of the figure. |
| `legend` | `Figure.legend(position : String, title : String) : void` | The legend of the figure. |
| `legendTitle` | `Figure.legendTitle(title : String) : void` | The legend title. |
| `legendColumns` | `Figure.legendColumns(columns : Int) : void` | How many columns the legend uses. |
| `legendFrame` | `Figure.legendFrame(enabled : Bool) : void` | Whether the legend has a frame. |
| `colors` | `Figure.colors(values : List[String]) : void` | The colours of the plot elements. |
| `fontFamily` | `Figure.fontFamily(family : String, kind : String) : void` | The font family of the figure. |
| `fontSize` | `Figure.fontSize(size : double) : void` | The font size of the figure. |
| `addLineValues` | `Figure.addLineValues(xs : List[double], ys : List[double], xLabels : List[String], label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double, step : String) : void` | Adds a line from raw arrays. |
| `plot` | `Figure.plot(y : List[double], label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double, step : String) : void` | Draws the values on the axes. |
| `plot` | `Figure.plot(y : List[Int], label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double, step : String) : void` | Draws the values on the axes. |
| `plot` | `Figure.plot(x : List[double], y : List[double], label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double, step : String) : void` | Draws the values on the axes. |
| `plot` | `Figure.plot(x : List[Int], y : List[double], label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double, step : String) : void` | Draws the values on the axes. |
| `plot` | `Figure.plot(x : List[double], y : List[Int], label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double, step : String) : void` | Draws the values on the axes. |
| `plot` | `Figure.plot(x : List[Int], y : List[Int], label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double, step : String) : void` | Draws the values on the axes. |
| `plot` | `Figure.plot(x : List[String], y : List[double], label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double, step : String) : void` | Draws the values on the axes. |
| `plot` | `Figure.plot(x : List[String], y : List[Int], label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double, step : String) : void` | Draws the values on the axes. |
| `step` | `Figure.step(x : List[double], y : List[double], where : String, label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double) : void` | A step plot. |
| `step` | `Figure.step(x : List[Int], y : List[double], where : String, label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double) : void` | A step plot. |
| `plotSeries` | `Figure.plotSeries(series : Series, label : String, color : String, lineWidth : double, lineStyle : String, marker : String, markerSize : double, alpha : double, step : String) : void` | Plots one series. |
| `addScatterValues` | `Figure.addScatterValues(xs : List[double], ys : List[double], xLabels : List[String], colors : List[String], sizes : List[double], label : String, color : String, marker : String, markerSize : double, alpha : double, edgeColor : String, edgeWidth : double) : void` | Adds points from raw arrays. |
| `scatter` | `Figure.scatter(x : List[double], y : List[double], label : String, color : String, marker : String, markerSize : double, alpha : double, edgeColor : String, edgeWidth : double) : void` | A scatter plot. |
| `scatter` | `Figure.scatter(x : List[Int], y : List[double], label : String, color : String, marker : String, markerSize : double, alpha : double, edgeColor : String, edgeWidth : double) : void` | A scatter plot. |
| `scatter` | `Figure.scatter(x : List[double], y : List[Int], label : String, color : String, marker : String, markerSize : double, alpha : double, edgeColor : String, edgeWidth : double) : void` | A scatter plot. |
| `scatter` | `Figure.scatter(x : List[Int], y : List[Int], label : String, color : String, marker : String, markerSize : double, alpha : double, edgeColor : String, edgeWidth : double) : void` | A scatter plot. |
| `scatter` | `Figure.scatter(x : List[String], y : List[double], label : String, color : String, marker : String, markerSize : double, alpha : double, edgeColor : String, edgeWidth : double) : void` | A scatter plot. |
| `scatter` | `Figure.scatter(x : List[double], y : List[double], sizes : List[double], label : String, color : String, marker : String, alpha : double, edgeColor : String, edgeWidth : double) : void` | A scatter plot. |
| `scatterSeries` | `Figure.scatterSeries(series : Series, label : String, color : String, marker : String, markerSize : double, alpha : double) : void` | A scatter plot of two series. |
| `addBarsValues` | `Figure.addBarsValues(positions : List[double], heights : List[double], bases : List[double], offset : double, width : double, horizontal : Bool, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double) : void` | Adds bars from raw arrays. |
| `bar` | `Figure.bar(labels : List[String], values : List[double], width : double, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double, offset : double) : void` | A bar chart. |
| `bar` | `Figure.bar(labels : List[String], values : List[Int], width : double, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double, offset : double) : void` | A bar chart. |
| `bar` | `Figure.bar(positions : List[double], values : List[double], width : double, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double, offset : double) : void` | A bar chart. |
| `bar` | `Figure.bar(positions : List[Int], values : List[double], width : double, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double, offset : double) : void` | A bar chart. |
| `barh` | `Figure.barh(labels : List[String], values : List[double], width : double, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double, offset : double) : void` | A horizontal bar chart. |
| `barh` | `Figure.barh(labels : List[String], values : List[Int], width : double, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double, offset : double) : void` | A horizontal bar chart. |
| `addBandValues` | `Figure.addBandValues(xs : List[double], lower : List[double], upper : List[double], xLabels : List[String], label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double, lineStyle : String) : void` | Draws a filled band from raw arrays. |
| `fillBetween` | `Figure.fillBetween(x : List[double], lower : List[double], upper : List[double], label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double, lineStyle : String) : void` | Fills the area between two series. |
| `fillBetween` | `Figure.fillBetween(x : List[Int], lower : List[double], upper : List[double], label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double, lineStyle : String) : void` | Fills the area between two series. |
| `fillBetween` | `Figure.fillBetween(x : List[double], lower : List[Int], upper : List[Int], label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double, lineStyle : String) : void` | Fills the area between two series. |
| `area` | `Figure.area(x : List[double], y : List[double], base : double, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double) : void` | Fills the area below a line. |
| `area` | `Figure.area(x : List[Int], y : List[double], base : double, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double) : void` | Fills the area below a line. |
| `plotErrorValues` | `Figure.plotErrorValues(xs : List[double], ys : List[double], yLow : List[double], yHigh : List[double], xLow : List[double], xHigh : List[double], xLabels : List[String], label : String, color : String, width : double, capSize : double, marker : String, markerSize : double, alpha : double) : void` | Draws error bars from raw arrays. |
| `errorBar` | `Figure.errorBar(x : List[double], y : List[double], yerr : List[double], label : String, color : String, width : double, capSize : double, marker : String, markerSize : double, alpha : double) : void` | Draws error bars. |
| `errorBar` | `Figure.errorBar(x : List[Int], y : List[double], yerr : List[double], label : String, color : String, width : double, capSize : double, marker : String, markerSize : double, alpha : double) : void` | Draws error bars. |
| `errorBar` | `Figure.errorBar(x : List[double], y : List[double], low : List[double], high : List[double], label : String, color : String, width : double, capSize : double, marker : String, markerSize : double, alpha : double) : void` | Draws error bars. |
| `errorBarSeries` | `Figure.errorBarSeries(series : Series, yerr : List[double], label : String, color : String, width : double, marker : String, markerSize : double) : void` | Error bars from a series. |
| `stem` | `Figure.stem(x : List[double], y : List[double], base : double, label : String, color : String, width : double, marker : String, markerSize : double, alpha : double) : void` | A stem plot of x against y. |
| `stem` | `Figure.stem(x : List[Int], y : List[double], base : double, label : String, color : String, width : double, marker : String, markerSize : double, alpha : double) : void` | A stem plot of x against y. |
| `hist` | `Figure.hist(values : List[double], bins : Int, density : Bool, cumulative : Bool, histType : String, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double) : void` | A histogram. |
| `hist` | `Figure.hist(values : List[Int], bins : Int, density : Bool, cumulative : Bool, histType : String, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double) : void` | A histogram. |
| `hist` | `Figure.hist(values : List[double], edges : List[double], density : Bool, cumulative : Bool, histType : String, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double) : void` | A histogram. |
| `histSeries` | `Figure.histSeries(series : Series, bins : Int, density : Bool, cumulative : Bool, histType : String, label : String, color : String, alpha : double) : void` | A histogram of one series. |
| `box` | `Figure.box(values : List[double], label : String, color : String, width : double) : void` | A box plot. |
| `box` | `Figure.box(values : List[Int], label : String, color : String, width : double) : void` | A box plot. |
| `boxPlot` | `Figure.boxPlot(groups : List[List[double]], labels : List[String], color : String, width : double) : void` | Draws a box plot. |
| `boxSeries` | `Figure.boxSeries(series : Series, label : String, color : String, width : double) : void` | A box plot of a series. |
| `heatmap` | `Figure.heatmap(matrix : List[List[double]], rowLabels : List[String], colLabels : List[String], cmap : String, colorbar : Bool, colorbarLabel : String, grid : Bool) : void` | Draws a heat map. |
| `heatmapScaled` | `Figure.heatmapScaled(matrix : List[List[double]], rowLabels : List[String], colLabels : List[String], cmap : String, vmin : double, vmax : double, colorbar : Bool, colorbarLabel : String) : void` | A heat map with scaled colours. |
| `imshow` | `Figure.imshow(matrix : List[List[double]], cmap : String, colorbar : Bool) : void` | Draws a matrix as an image. |
| `pie` | `Figure.pie(values : List[double], labels : List[String], colors : List[String], startAngle : double, explode : List[double], donut : double, autoPercent : Bool, labelDistance : double, showLabels : Bool, edgeColor : String, edgeWidth : double) : void` | A pie chart. |
| `pie` | `Figure.pie(values : List[Int], labels : List[String], colors : List[String], startAngle : double, explode : List[double], donut : double, autoPercent : Bool, showLabels : Bool) : void` | A pie chart. |
| `text` | `Figure.text(x : double, y : double, content : String, color : String, size : double, align : String, rotation : double, weight : String, coordinate : String, family : String, alpha : double) : void` | Writes text at a position. |
| `textAxes` | `Figure.textAxes(x : double, y : double, content : String, color : String, size : double, align : String, rotation : double, weight : String, family : String, alpha : double) : void` | Writes text on the axes. |
| `toSvgFallback` | `Figure.toSvgFallback() : String` | The figure as SVG, without optional fonts. |
| `toSvg` | `Figure.toSvg() : String or Error with Effects` | The figure as SVG text. |
| `toSvgTex` | `Figure.toSvgTex() : String or Error with Effects` | The figure as SVG with TeX labels. |
| `toHtml` | `Figure.toHtml() : String or Error with Effects` | The frame as an HTML table. |
| `toHtmlPage` | `Figure.toHtmlPage() : String or Error with Effects` | The frame as a complete HTML page. |
| `html` | `Figure.html() : String with Effects` | The frame as an HTML table. |
| `text` | `Figure.text() : String` | Writes text at a position. |
| `save` | `Figure.save(path : String, dpi : Int) : void or Error with Effects` | Writes the figure to a file. |
| `plotFrame` | `Figure.plotFrame(frame : DataFrame, x : String, y : List[String], kind : String, colors : List[String], labels : List[String], alpha : double, lineWidth : double, marker : String, stacked : Bool, edgeColor : String, edgeWidth : double) : void or Error` | Plots a whole frame. |
| `plotFrame` | `Figure.plotFrame(frame : DataFrame, x : String, y : String, kind : String, colors : List[String], labels : List[String], alpha : double, lineWidth : double, marker : String, stacked : Bool, edgeColor : String, edgeWidth : double) : void or Error` | Plots a whole frame. |
| `barFrame` | `Figure.barFrame(frame : DataFrame, x : String, y : List[String], stacked : Bool, horizontal : Bool, colors : List[String], labels : List[String], alpha : double) : void or Error` | A bar chart of a frame. |
| `barFrame` | `Figure.barFrame(frame : DataFrame, x : String, y : String, stacked : Bool, horizontal : Bool, colors : List[String], labels : List[String], alpha : double) : void or Error` | A bar chart of a frame. |
| `barhFrame` | `Figure.barhFrame(frame : DataFrame, x : String, y : List[String], stacked : Bool, colors : List[String], labels : List[String]) : void or Error` | A horizontal bar chart of a frame. |
| `areaFrame` | `Figure.areaFrame(frame : DataFrame, x : String, y : List[String], colors : List[String], labels : List[String], alpha : double) : void or Error` | An area plot of a frame. |
| `stepFrame` | `Figure.stepFrame(frame : DataFrame, x : String, y : List[String], colors : List[String], labels : List[String]) : void or Error` | A step plot of a frame. |
| `scatterFrame` | `Figure.scatterFrame(frame : DataFrame, x : String, y : String, colorBy : String, sizeBy : String, marker : String, markerSize : double, alpha : double, label : String, cmap : String) : void or Error` | A scatter plot of two frame columns. |
| `histFrame` | `Figure.histFrame(frame : DataFrame, columns : List[String], bins : Int, density : Bool, cumulative : Bool, histType : String, labels : List[String], colors : List[String], alpha : double) : void or Error` | Histograms of every column. |
| `histFrame` | `Figure.histFrame(frame : DataFrame, column : String, bins : Int, density : Bool, cumulative : Bool, histType : String, labels : List[String], colors : List[String], alpha : double) : void or Error` | Histograms of every column. |
| `boxFrame` | `Figure.boxFrame(frame : DataFrame, columns : List[String], labels : List[String], color : String, width : double) : void or Error` | A box plot of a frame. |
| `heatmapFrame` | `Figure.heatmapFrame(frame : DataFrame, columns : List[String], cmap : String, colorbar : Bool, colorbarLabel : String) : void or Error` | A heat map of a frame. |
| `pieFrame` | `Figure.pieFrame(frame : DataFrame, values : String, labels : String, colors : List[String], startAngle : double, donut : double, autoPercent : Bool) : void or Error` | A pie chart of a frame column. |
| `lineSeries` | `Figure.lineSeries(series : Series, label : String, color : String) : void` | A line plot of a series. |
| `hLine` | `Figure.hLine(y : double, label : String, color : String, width : double, lineStyle : String, alpha : double) : void` | A horizontal line at a y value. |
| `vLine` | `Figure.vLine(x : double, label : String, color : String, width : double, lineStyle : String, alpha : double) : void` | See the signature. |
| `axhline` | `Figure.axhline(y : double, label : String, color : String, width : double, lineStyle : String, alpha : double) : void` | A horizontal line across the axes. |
| `axvline` | `Figure.axvline(x : double, label : String, color : String, width : double, lineStyle : String, alpha : double) : void` | A vertical line across the axes. |
| `hLines` | `Figure.hLines(values : List[double], label : String, color : String, width : double, lineStyle : String, alpha : double) : void` | Horizontal lines at several y values. |
| `vLines` | `Figure.vLines(values : List[double], label : String, color : String, width : double, lineStyle : String, alpha : double) : void` | See the signature. |
| `stackPlot` | `Figure.stackPlot(x : List[double], series : List[List[double]], labels : List[String], colors : List[String], alpha : double, lineWidth : double) : void` | A stacked area plot. |
| `stackPlotFrame` | `Figure.stackPlotFrame(frame : DataFrame, x : String, y : List[String], labels : List[String], colors : List[String], alpha : double) : void or Error` | A stacked area plot of a frame. |
| `panelLabel` | `Figure.panelLabel(text : String) : void` | The label of a panel. |
| `clabel` | `Figure.clabel(text : String) : void` | Labels the contours. |
| `errorBarXY` | `Figure.errorBarXY(x : List[double], y : List[double], xerr : List[double], yerr : List[double], label : String, color : String, width : double, capSize : double, marker : String, markerSize : double, alpha : double) : void` | Error bars from two arrays. |
| `barh` | `Figure.barh(positions : List[double], values : List[double], width : double, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double) : void` | A horizontal bar chart. |
| `barh` | `Figure.barh(positions : List[Int], values : List[double], width : double, label : String, color : String, alpha : double, edgeColor : String, edgeWidth : double) : void` | A horizontal bar chart. |
| `plot` | `Figure.plot(x : List[double], series : List[List[double]], labels : List[String], colors : List[String]) : void` | Draws the values on the axes. |
| `plot` | `Figure.plot(series : List[List[double]], labels : List[String], colors : List[String]) : void` | Draws the values on the axes. |
| `contour` | `Figure.contour(matrix : List[List[double]], levels : Int, cmap : String, lineWidth : double, rowLabels : List[String], colLabels : List[String], colorbar : Bool, colorbarLabel : String) : void` | A contour plot. |
| `contour` | `Figure.contour(matrix : List[List[double]], levels : List[double], cmap : String, lineWidth : double, rowLabels : List[String], colLabels : List[String], colorbar : Bool, colorbarLabel : String) : void` | A contour plot. |
| `contourFrame` | `Figure.contourFrame(frame : DataFrame, columns : List[String], levels : Int, cmap : String, lineWidth : double, colorbar : Bool, colorbarLabel : String) : void or Error` | A contour plot of a frame. |
| `annotate` | `Figure.annotate(text : String, xyX : double, xyY : double, textX : double, textY : double, color : String, size : double, align : String, weight : String, arrowColor : String, arrowWidth : double, alpha : double) : void` | Writes a text annotation at a data point. |
| `arrow` | `Figure.arrow(x1 : double, y1 : double, x2 : double, y2 : double, color : String, width : double, alpha : double) : void` | Draws an arrow on the axes. |
| `plot` | `Figure.plot(frame : DataFrame, x : String, y : List[String], kind : String, colors : List[String], labels : List[String], alpha : double, lineWidth : double, marker : String, stacked : Bool) : void or Error` | Draws the values on the axes. |
| `plot` | `Figure.plot(frame : DataFrame, x : String, y : String, kind : String, colors : List[String], labels : List[String], alpha : double, lineWidth : double, marker : String, stacked : Bool) : void or Error` | Draws the values on the axes. |
| `bar` | `Figure.bar(frame : DataFrame, x : String, y : List[String], stacked : Bool, horizontal : Bool, colors : List[String], labels : List[String], alpha : double) : void or Error` | A bar chart. |
| `bar` | `Figure.bar(frame : DataFrame, x : String, y : String, stacked : Bool, horizontal : Bool, colors : List[String], labels : List[String], alpha : double) : void or Error` | A bar chart. |
| `scatter` | `Figure.scatter(frame : DataFrame, x : String, y : String, colorBy : String, sizeBy : String, marker : String, markerSize : double, alpha : double, label : String, cmap : String) : void or Error` | A scatter plot. |
| `hist` | `Figure.hist(frame : DataFrame, columns : List[String], bins : Int, density : Bool, cumulative : Bool, histType : String, labels : List[String], colors : List[String], alpha : double) : void or Error` | A histogram. |
| `hist` | `Figure.hist(frame : DataFrame, column : String, bins : Int, density : Bool, cumulative : Bool, histType : String, labels : List[String], colors : List[String], alpha : double) : void or Error` | A histogram. |
| `pie` | `Figure.pie(frame : DataFrame, values : String, labels : String, colors : List[String], startAngle : double, donut : double, autoPercent : Bool) : void or Error` | A pie chart. |
| `savefig` | `Figure.savefig(path : String, dpi : Int) : void or Error with Effects` | Writes the figure as an image. |
| `suptitle` | `Figure.suptitle(text : String, size : double) : void` | The figure title. |
