/* ═══════════════════════════════════════════════════════════════
   HangarAI site: a QR code, drawn in the page

   For the referral link in the account's "Invite & earn": nothing is
   loaded from another origin, so the code is made here. ISO/IEC 18004,
   byte mode, versions 1 to 10 (up to 213 bytes at level M, far more
   than a link needs), the eight masks scored as the standard says.
   After Project Nayuki's QR Code generator (MIT), cut down to that.

     HangarQR.make(text, 'M') → { size, dark(x, y) }
     HangarQR.path(qr, border) → the SVG path of the dark modules
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  // Per level (L, M, Q, H), by version (index 0 unused).
  var ECC_PER_BLOCK = {
    L: [0, 7, 10, 15, 20, 26, 18, 20, 24, 30, 18],
    M: [0, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26],
    Q: [0, 13, 22, 18, 26, 18, 24, 18, 22, 20, 24],
    H: [0, 17, 28, 22, 16, 22, 28, 26, 26, 24, 28]
  };
  var BLOCKS = {
    L: [0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 4],
    M: [0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5],
    Q: [0, 1, 1, 2, 2, 4, 4, 6, 6, 8, 8],
    H: [0, 1, 1, 2, 4, 4, 4, 5, 6, 8, 8]
  };
  var FORMAT = { L: 1, M: 0, Q: 3, H: 2 };
  var MAX_VERSION = 10;

  /* ── Reed-Solomon over GF(2^8), polynomial 0x11D ─────────────── */

  function mul(x, y) {
    var z = 0;
    for (var i = 7; i >= 0; i--) {
      z = (z << 1) ^ ((z >>> 7) * 0x11d);
      z ^= ((y >>> i) & 1) * x;
    }
    return z;
  }

  function divisor(degree) {
    var result = [];
    for (var i = 0; i < degree - 1; i++) result.push(0);
    result.push(1);
    var root = 1;
    for (i = 0; i < degree; i++) {
      for (var j = 0; j < result.length; j++) {
        result[j] = mul(result[j], root);
        if (j + 1 < result.length) result[j] ^= result[j + 1];
      }
      root = mul(root, 0x02);
    }
    return result;
  }

  function remainder(data, div) {
    var result = div.map(function () { return 0; });
    data.forEach(function (b) {
      var factor = b ^ result.shift();
      result.push(0);
      div.forEach(function (coef, i) { result[i] ^= mul(coef, factor); });
    });
    return result;
  }

  /* ── sizes ───────────────────────────────────────────────────── */

  function rawModules(ver) {
    var result = (16 * ver + 128) * ver + 64;
    if (ver >= 2) {
      var n = Math.floor(ver / 7) + 2;
      result -= (25 * n - 10) * n - 55;
      if (ver >= 7) result -= 36;
    }
    return result;
  }

  function dataCodewords(ver, ecl) {
    return Math.floor(rawModules(ver) / 8) - ECC_PER_BLOCK[ecl][ver] * BLOCKS[ecl][ver];
  }

  function alignment(ver) {
    if (ver === 1) return [];
    var n = Math.floor(ver / 7) + 2;
    var step = Math.ceil((ver * 4 + 4) / (n * 2 - 2)) * 2;
    var result = [6];
    for (var pos = ver * 4 + 10; result.length < n; pos -= step) result.splice(1, 0, pos);
    return result;
  }

  /* ── the data: byte mode, then the error correction ──────────── */

  function utf8(text) {
    return Array.prototype.slice.call(new TextEncoder().encode(text));
  }

  function codewords(bytes, ver, ecl) {
    var bits = [];
    function put(value, len) {
      for (var i = len - 1; i >= 0; i--) bits.push((value >>> i) & 1);
    }
    put(4, 4);
    put(bytes.length, ver <= 9 ? 8 : 16);
    bytes.forEach(function (b) { put(b, 8); });
    var capacity = dataCodewords(ver, ecl) * 8;
    put(0, Math.min(4, capacity - bits.length));
    put(0, (8 - bits.length % 8) % 8);
    for (var pad = 0xec; bits.length < capacity; pad ^= 0xec ^ 0x11) put(pad, 8);
    var data = [];
    for (var i = 0; i < bits.length; i += 8) {
      var b = 0;
      for (var j = 0; j < 8; j++) b = (b << 1) | bits[i + j];
      data.push(b);
    }

    // Split into blocks, add each one's error correction, interleave.
    var numBlocks = BLOCKS[ecl][ver];
    var eccLen = ECC_PER_BLOCK[ecl][ver];
    var raw = Math.floor(rawModules(ver) / 8);
    var shortBlocks = numBlocks - raw % numBlocks;
    var shortLen = Math.floor(raw / numBlocks);
    var div = divisor(eccLen);
    var blocks = [];
    for (i = 0, j = 0; i < numBlocks; i++) {
      var dat = data.slice(j, j + shortLen - eccLen + (i < shortBlocks ? 0 : 1));
      j += dat.length;
      var ecc = remainder(dat, div);
      if (i < shortBlocks) dat.push(0);
      blocks.push(dat.concat(ecc));
    }
    var result = [];
    for (i = 0; i < blocks[0].length; i++) {
      for (j = 0; j < blocks.length; j++) {
        if (i !== shortLen - eccLen || j >= shortBlocks) result.push(blocks[j][i]);
      }
    }
    return result;
  }

  /* ── the matrix ──────────────────────────────────────────────── */

  function Matrix(ver) {
    this.ver = ver;
    this.size = ver * 4 + 17;
    this.mod = [];
    this.fn = [];
    for (var y = 0; y < this.size; y++) {
      this.mod.push(new Array(this.size).fill(false));
      this.fn.push(new Array(this.size).fill(false));
    }
  }

  Matrix.prototype.set = function (x, y, dark) {
    this.mod[y][x] = dark;
    this.fn[y][x] = true;
  };

  Matrix.prototype.functions = function () {
    var size = this.size;
    var i;
    for (i = 0; i < size; i++) {
      this.set(6, i, i % 2 === 0);
      this.set(i, 6, i % 2 === 0);
    }
    var self = this;
    [[3, 3], [size - 4, 3], [3, size - 4]].forEach(function (c) {
      for (var dy = -4; dy <= 4; dy++) {
        for (var dx = -4; dx <= 4; dx++) {
          var d = Math.max(Math.abs(dx), Math.abs(dy));
          var x = c[0] + dx;
          var y = c[1] + dy;
          if (x >= 0 && x < size && y >= 0 && y < size) self.set(x, y, d !== 2 && d !== 4);
        }
      }
    });
    var pos = alignment(this.ver);
    var n = pos.length;
    for (i = 0; i < n; i++) {
      for (var j = 0; j < n; j++) {
        if ((i === 0 && j === 0) || (i === 0 && j === n - 1) || (i === n - 1 && j === 0)) continue;
        for (var dy = -2; dy <= 2; dy++) {
          for (var dx = -2; dx <= 2; dx++) self.set(pos[i] + dx, pos[j] + dy, Math.max(Math.abs(dx), Math.abs(dy)) !== 1);
        }
      }
    }
    this.format('M', 0);
    if (this.ver >= 7) {
      var rem = this.ver;
      for (i = 0; i < 12; i++) rem = (rem << 1) ^ ((rem >>> 11) * 0x1f25);
      var bits = (this.ver << 12) | rem;
      for (i = 0; i < 18; i++) {
        var bit = ((bits >>> i) & 1) === 1;
        var a = size - 11 + i % 3;
        var b = Math.floor(i / 3);
        this.set(a, b, bit);
        this.set(b, a, bit);
      }
    }
  };

  Matrix.prototype.format = function (ecl, mask) {
    var data = (FORMAT[ecl] << 3) | mask;
    var rem = data;
    for (var i = 0; i < 10; i++) rem = (rem << 1) ^ ((rem >>> 9) * 0x537);
    var bits = ((data << 10) | rem) ^ 0x5412;
    function bit(k) { return ((bits >>> k) & 1) === 1; }
    var size = this.size;
    for (i = 0; i <= 5; i++) this.set(8, i, bit(i));
    this.set(8, 7, bit(6));
    this.set(8, 8, bit(7));
    this.set(7, 8, bit(8));
    for (i = 9; i < 15; i++) this.set(14 - i, 8, bit(i));
    for (i = 0; i < 8; i++) this.set(size - 1 - i, 8, bit(i));
    for (i = 8; i < 15; i++) this.set(8, size - 15 + i, bit(i));
    this.set(8, size - 8, true);
  };

  Matrix.prototype.place = function (data) {
    var size = this.size;
    var i = 0;
    for (var right = size - 1; right >= 1; right -= 2) {
      if (right === 6) right = 5;
      for (var vert = 0; vert < size; vert++) {
        for (var j = 0; j < 2; j++) {
          var x = right - j;
          var upward = ((right + 1) & 2) === 0;
          var y = upward ? size - 1 - vert : vert;
          if (!this.fn[y][x] && i < data.length * 8) {
            this.mod[y][x] = ((data[i >>> 3] >>> (7 - (i & 7))) & 1) === 1;
            i++;
          }
        }
      }
    }
  };

  var MASKS = [
    function (x, y) { return (x + y) % 2 === 0; },
    function (x, y) { return y % 2 === 0; },
    function (x) { return x % 3 === 0; },
    function (x, y) { return (x + y) % 3 === 0; },
    function (x, y) { return (Math.floor(x / 3) + Math.floor(y / 2)) % 2 === 0; },
    function (x, y) { return (x * y) % 2 + (x * y) % 3 === 0; },
    function (x, y) { return ((x * y) % 2 + (x * y) % 3) % 2 === 0; },
    function (x, y) { return ((x + y) % 2 + (x * y) % 3) % 2 === 0; }
  ];

  Matrix.prototype.mask = function (m) {
    for (var y = 0; y < this.size; y++) {
      for (var x = 0; x < this.size; x++) {
        if (!this.fn[y][x] && MASKS[m](x, y)) this.mod[y][x] = !this.mod[y][x];
      }
    }
  };

  // The standard's four penalties: runs, 2×2 blocks, finder-like
  // patterns, and the balance of dark and light.
  Matrix.prototype.penalty = function () {
    var size = this.size;
    var mod = this.mod;
    var score = 0;
    var dark = 0;
    var FINDER = [true, false, true, true, true, false, true, false, false, false, false];
    function line(get) {
      var run = 1;
      for (var i = 1; i <= size; i++) {
        if (i < size && get(i) === get(i - 1)) {
          run++;
        } else {
          if (run >= 5) score += 3 + run - 5;
          run = 1;
        }
      }
      for (i = 0; i + 11 <= size; i++) {
        var fwd = true;
        var back = true;
        for (var k = 0; k < 11; k++) {
          if (get(i + k) !== FINDER[k]) fwd = false;
          if (get(i + k) !== FINDER[10 - k]) back = false;
        }
        if (fwd) score += 40;
        if (back) score += 40;
      }
    }
    for (var y = 0; y < size; y++) {
      line(function (x) { return mod[y][x]; });
      line(function (x) { return mod[x][y]; });
    }
    for (y = 0; y < size - 1; y++) {
      for (var x = 0; x < size - 1; x++) {
        var c = mod[y][x];
        if (c === mod[y][x + 1] && c === mod[y + 1][x] && c === mod[y + 1][x + 1]) score += 3;
      }
    }
    for (y = 0; y < size; y++) for (x = 0; x < size; x++) if (mod[y][x]) dark++;
    var total = size * size;
    score += (Math.ceil(Math.abs(dark * 20 - total * 10) / total) - 1) * 10;
    return score;
  };

  /* ── the code ────────────────────────────────────────────────── */

  function make(text, ecl) {
    ecl = FORMAT[ecl] !== undefined ? ecl : 'M';
    var bytes = utf8(text);
    var ver = 1;
    while (ver <= MAX_VERSION && 4 + (ver <= 9 ? 8 : 16) + bytes.length * 8 > dataCodewords(ver, ecl) * 8) ver++;
    if (ver > MAX_VERSION) throw new Error('too long for a QR code of version 10');
    // The best level the version allows, at no cost in size.
    ['Q', 'H'].forEach(function (better) {
      if (FORMAT[better] !== undefined && rank(better) > rank(ecl) &&
        4 + (ver <= 9 ? 8 : 16) + bytes.length * 8 <= dataCodewords(ver, better) * 8) ecl = better;
    });
    var data = codewords(bytes, ver, ecl);
    var best = null;
    var bestScore = Infinity;
    for (var m = 0; m < 8; m++) {
      var mat = new Matrix(ver);
      mat.functions();
      mat.place(data);
      mat.mask(m);
      mat.format(ecl, m);
      var s = mat.penalty();
      if (s < bestScore) {
        best = mat;
        bestScore = s;
      }
    }
    return {
      size: best.size,
      version: ver,
      level: ecl,
      dark: function (x, y) { return x >= 0 && y >= 0 && x < best.size && y < best.size && best.mod[y][x]; }
    };
  }

  function rank(ecl) {
    return { L: 0, M: 1, Q: 2, H: 3 }[ecl];
  }

  // One path for every dark module, a square each, with `border`
  // modules of quiet zone around (the viewBox is size + 2 × border).
  function path(qr, border) {
    var b = border === undefined ? 4 : border;
    var parts = [];
    for (var y = 0; y < qr.size; y++) {
      for (var x = 0; x < qr.size; x++) {
        if (qr.dark(x, y)) parts.push('M' + (x + b) + ' ' + (y + b) + 'h1v1h-1z');
      }
    }
    return parts.join('');
  }

  window.HangarQR = { make: make, path: path };
})();
