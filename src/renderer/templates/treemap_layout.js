
  /* ---- Treemap layout ---- */

  /* Recursively sum LOC into each directory node's totalLoc. */
  function tmComputeLoc(node) {
    var sum = 0;
    node.files.forEach(function(f) { sum += f.loc; });
    Object.keys(node.children).forEach(function(k) { sum += tmComputeLoc(node.children[k]); });
    node.totalLoc = sum;
    return sum;
  }

  /* Collapse single-child directory chains (a/b/c) into one node. */
  function tmSquashSingle(node) {
    var keys = Object.keys(node.children);
    keys.forEach(function(k) { tmSquashSingle(node.children[k]); });
    keys = Object.keys(node.children);
    if (keys.length === 1 && node.files.length === 0 && node.name !== '/') {
      var child = node.children[keys[0]];
      node.name = node.name + '/' + child.name;
      node.children = child.children;
      node.files = child.files;
    }
  }

  function buildFileTree(files) {
    var root = { name: '/', children: {}, files: [], totalLoc: 0 };
    files.forEach(function(f) {
      var parts = f.path.split('/');
      var fname = parts.pop();
      var node = root;
      parts.forEach(function(p) {
        if (!node.children[p]) {
          node.children[p] = { name: p, children: {}, files: [], totalLoc: 0 };
        }
        node = node.children[p];
      });
      node.files.push({ name: fname, path: f.path, loc: f.loc });
    });
    tmComputeLoc(root);
    tmSquashSingle(root);
    return root;
  }

  /* Lay one squarified row into `rects`, filling the short side of the
     free rectangle. Returns the remaining free rectangle. `norm` carries
     the layout's totalSize/totalArea normalisation. */
  function tmLayoutRow(rects, row, rowSize, rx, ry, rw, rh, norm) {
    var short = Math.min(rw, rh);
    var rowArea = (rowSize / norm.totalSize) * norm.totalArea;
    var rowLen = short > 0 ? rowArea / short : 0;
    var offset = 0;
    var horizontal = rw >= rh;
    row.forEach(function(it) {
      var frac = rowSize > 0 ? it.size / rowSize : 0;
      var itemLen = frac * short;
      if (horizontal) {
        rects.push({ x: rx, y: ry + offset, w: rowLen, h: itemLen, data: it.data });
      } else {
        rects.push({ x: rx + offset, y: ry, w: itemLen, h: rowLen, data: it.data });
      }
      offset += itemLen;
    });
    if (horizontal) {
      return { x: rx + rowLen, y: ry, w: rw - rowLen, h: rh };
    }
    return { x: rx, y: ry + rowLen, w: rw, h: rh - rowLen };
  }

  /* Worst aspect ratio in a candidate row — the squarify criterion. */
  function tmWorstRatio(row, rowSize, short, norm) {
    if (row.length === 0 || short <= 0) return Infinity;
    var rowArea = (rowSize / norm.totalSize) * norm.totalArea;
    var worst = 0;
    row.forEach(function(it) {
      var frac = it.size / rowSize;
      var itemArea = frac * rowArea;
      var itemLen = short > 0 ? frac * short : 0;
      var itemWidth = itemLen > 0 ? itemArea / itemLen : 0;
      var r = itemWidth > itemLen ? itemWidth / itemLen : itemLen / itemWidth;
      if (r > worst) worst = r;
    });
    return worst;
  }

  function squarify(items, x, y, w, h) {
    if (items.length === 0) return [];
    var results = [];
    var remaining = items.slice().sort(function(a, b) { return b.size - a.size; });
    var totalSize = 0;
    remaining.forEach(function(it) { totalSize += it.size; });
    if (totalSize <= 0) return [];
    var norm = { totalSize: totalSize, totalArea: w * h };

    var rx = x, ry = y, rw = w, rh = h;
    while (remaining.length > 0) {
      var short = Math.min(rw, rh);
      if (short <= 0) break;
      var row = [remaining[0]];
      var rowSize = remaining[0].size;
      remaining.splice(0, 1);
      var currentWorst = tmWorstRatio(row, rowSize, short, norm);

      // Grow the row while it improves (or keeps) the worst aspect ratio
      while (remaining.length > 0) {
        var next = remaining[0];
        var newSize = rowSize + next.size;
        var newRow = row.concat([next]);
        var newWorst = tmWorstRatio(newRow, newSize, short, norm);
        if (newWorst > currentWorst) break;
        row = newRow;
        rowSize = newSize;
        currentWorst = newWorst;
        remaining.splice(0, 1);
      }
      var rest = tmLayoutRow(results, row, rowSize, rx, ry, rw, rh, norm);
      rx = rest.x; ry = rest.y; rw = rest.w; rh = rest.h;
    }
    return results;
  }

  function circlePack(items, cx, cy, r) {
    if (items.length === 0) return [];
    var sorted = items.slice().sort(function(a, b) { return b.size - a.size; });
    var totalSize = 0;
    sorted.forEach(function(it) { totalSize += it.size; });
    if (totalSize <= 0) return [];

    // Assign radii proportional to sqrt(size) so area ~ size
    var radii = [];
    var sumR2 = 0;
    sorted.forEach(function(it) {
      var ri = Math.sqrt(it.size / totalSize);
      radii.push(ri);
      sumR2 += ri * ri;
    });
    // Scale so circles fit inside parent radius with padding
    var scale = r * 0.85 / Math.sqrt(sumR2);
    radii = radii.map(function(ri) { return ri * scale; });

    // Place circles using simple greedy front-chain approach
    var placed = [];
    for (var i = 0; i < sorted.length; i++) {
      var ri = Math.max(radii[i], 2);
      if (i === 0) {
        placed.push({ cx: cx, cy: cy, r: ri, data: sorted[i].data });
      } else if (i === 1) {
        placed.push({ cx: cx + placed[0].r + ri + 1, cy: cy, r: ri, data: sorted[i].data });
      } else {
        var best = tmBestCirclePosition(placed, ri, cx, cy, r);
        placed.push({ cx: best.x, cy: best.y, r: ri, data: sorted[i].data });
      }
    }

    // Center the packed circles within the parent
    if (placed.length > 0) {
      var avgX = 0, avgY = 0;
      placed.forEach(function(p) { avgX += p.cx; avgY += p.cy; });
      avgX /= placed.length;
      avgY /= placed.length;
      var shiftX = cx - avgX, shiftY = cy - avgY;
      placed.forEach(function(p) { p.cx += shiftX; p.cy += shiftY; });
    }

    return placed;
  }

  /* True when a circle of radius `ri` at (px, py) would overlap any
     already-placed circle. */
  function tmOverlapsAny(placed, px, py, ri) {
    for (var m = 0; m < placed.length; m++) {
      var dx = px - placed[m].cx, dy = py - placed[m].cy;
      if (Math.sqrt(dx * dx + dy * dy) < ri + placed[m].r - 0.5) return true;
    }
    return false;
  }

  /* Best non-overlapping position for a new circle of radius `ri`: try
     every tangent point of every placed pair, keep the candidate closest
     to the parent centre that stays inside the parent radius. Falls back
     to the centre when no candidate fits. */
  function tmBestCirclePosition(placed, ri, cx, cy, r) {
    var bestX = cx, bestY = cy, bestDist = Infinity;
    for (var j = 0; j < placed.length; j++) {
      for (var k = j + 1; k < placed.length; k++) {
        var candidates = tangentPositions(placed[j], placed[k], ri);
        for (var c = 0; c < candidates.length; c++) {
          var px = candidates[c].x, py = candidates[c].y;
          var dist = Math.sqrt((px - cx) * (px - cx) + (py - cy) * (py - cy));
          if (dist + ri > r * 0.95) continue; // outside parent
          if (tmOverlapsAny(placed, px, py, ri)) continue;
          if (dist < bestDist) {
            bestDist = dist;
            bestX = px;
            bestY = py;
          }
        }
      }
    }
    return { x: bestX, y: bestY };
  }

  function tangentPositions(c1, c2, r) {
    var dx = c2.cx - c1.cx, dy = c2.cy - c1.cy;
    var d = Math.sqrt(dx * dx + dy * dy);
    if (d < 0.001) return [{ x: c1.cx + c1.r + r, y: c1.cy }];
    var d1 = c1.r + r, d2 = c2.r + r;
    if (d > d1 + d2) return [];
    var a = (d1 * d1 - d2 * d2 + d * d) / (2 * d);
    var h2 = d1 * d1 - a * a;
    if (h2 < 0) h2 = 0;
    var h = Math.sqrt(h2);
    var mx = c1.cx + a * dx / d, my = c1.cy + a * dy / d;
    return [
      { x: mx + h * dy / d, y: my - h * dx / d },
      { x: mx - h * dy / d, y: my + h * dx / d }
    ];
  }

  var metricScales = {
    hotspot: {
      label: 'Hotspot Score',
      color: function(f) {
        var s = f.hotspot_score || 0;
        var t = Math.min(s, 100) / 100;
        var h = (1 - t) * 120;
        return 'hsl(' + h + ',80%,' + (35 + t * 15) + '%)';
      },
      legend: function() {
        return [
          { label: 'Low', color: 'hsl(120,80%,35%)' },
          { label: 'Medium', color: 'hsl(60,80%,42%)' },
          { label: 'High', color: 'hsl(0,80%,50%)' }
        ];
      }
    },
    complexity: {
      label: 'Cyclomatic Complexity',
      color: function(f, maxCC) {
        var t = maxCC > 0 ? Math.min(f.cyclomatic_complexity || 0, maxCC) / maxCC : 0;
        return 'hsl(0,70%,' + (75 - t * 40) + '%)';
      },
      legend: function() {
        return [
          { label: 'Low', color: 'hsl(0,70%,75%)' },
          { label: 'High', color: 'hsl(0,70%,35%)' }
        ];
      }
    },
    churn: {
      label: 'Churn Count',
      color: function(f, _mc, maxChurn) {
        var t = maxChurn > 0 ? Math.min(f.churn_count || 0, maxChurn) / maxChurn : 0;
        return 'hsl(30,80%,' + (75 - t * 40) + '%)';
      },
      legend: function() {
        return [
          { label: 'Low', color: 'hsl(30,80%,75%)' },
          { label: 'High', color: 'hsl(30,80%,35%)' }
        ];
      }
    },
    age: {
      label: 'File Age (days)',
      color: function(f, _mc, _mch, ageMap, maxAge) {
        var a = ageMap[f.path];
        var days = a ? a.days_since_modified : 0;
        var t = maxAge > 0 ? Math.min(days, maxAge) / maxAge : 0;
        return 'hsl(220,70%,' + (75 - t * 40) + '%)';
      },
      legend: function() {
        return [
          { label: 'Recent', color: 'hsl(220,70%,75%)' },
          { label: 'Old', color: 'hsl(220,70%,35%)' }
        ];
      }
    },
    owner: {
      label: 'Top Contributor',
      color: function(f, _mc, _mch, _am, _ma, ownerMap, authorIndex) {
        var own = ownerMap[f.path];
        if (own && own.authors && own.authors[0]) {
          var idx = authorIndex[own.authors[0].name];
          return PALETTE[idx != null ? idx % PALETTE.length : 0];
        }
        return '#334155';
      },
      legend: function(authorIndex) {
        var items = [];
        for (var name in authorIndex) {
          items.push({ label: name, color: PALETTE[authorIndex[name] % PALETTE.length] });
        }
        return items;
      }
    }
  };
