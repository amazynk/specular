// Runs the gallery page's script against a small stand-in for the browser's DOM (Node.js,
// no packages): hearts are clicked, the picks are downloaded as picks.txt and picks.json,
// the page is "reloaded" with the same storage, and the lightbox is driven by keys. Prints
// what came out as JSON for tests/gallery.rs.
//
//     node fake_dom.js gallery.js case.json
"use strict";
const fs = require("fs");
const vm = require("vm");

const [script, casePath] = process.argv.slice(2);
const code = fs.readFileSync(script, "utf8");
const c = JSON.parse(fs.readFileSync(casePath, "utf8"));

function el(tag, attrs) {
  const e = {
    tagName: tag, attrs: Object.assign({}, attrs || {}), listeners: {}, hidden: false,
    disabled: false, textContent: "", value: "", classes: {}, q: {}, qa: {},
  };
  e.getAttribute = (k) => (k in e.attrs ? e.attrs[k] : null);
  e.setAttribute = (k, v) => { e.attrs[k] = String(v); };
  e.removeAttribute = (k) => { delete e.attrs[k]; };
  e.classList = {
    toggle: (k, on) => { e.classes[k] = !!on; },
    add: (k) => { e.classes[k] = true; },
    remove: (k) => { delete e.classes[k]; },
  };
  e.addEventListener = (t, f) => { (e.listeners[t] = e.listeners[t] || []).push(f); };
  e.fire = (t, ev) => (e.listeners[t] || []).forEach((f) => f(ev));
  e.focus = () => { doc.activeElement = e; };
  e.select = () => {};
  e.querySelector = (s) => e.q[s] || null;
  e.querySelectorAll = (s) => e.qa[s] || [];
  e.appendChild = () => {};
  e.removeChild = () => {};
  e.closest = () => (e.tagName === "button" ? e : null);
  e.click = () => { if (e.tagName === "a") { downloads[e.download] = blobs[e.href]; } };
  return e;
}
function click(target) {
  return { target, button: 0, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false,
    preventDefault() {} };
}

const downloads = {};
const blobs = {};
let nblob = 0;
const store = {};
const doc = {};

function page() {
  const body = el("body");
  body.dataset = { gallery: c.id, title: c.title, lPicked: "Picked: {n}", lPick: "Pick",
    lCopied: "Copied", lCopyHand: "By hand", lClearAsk: "Clear?", lNoStorage: "No storage",
    lPosition: "{i} of {n}" };
  const tiles = c.names.map((n, i) => {
    const t = el("li", { "data-name": n, "data-src": "images/" + i + ".jpg", "data-w": "10",
      "data-h": "10", "data-label": n, "data-exif": "" });
    t.q[".pick"] = el("button", { "aria-pressed": "false" });
    t.q[".open"] = el("a");
    t.q["img"] = el("img", { alt: n });
    return t;
  });
  const lb = el("div");
  for (const k of [".lb-img", ".lb-cap", ".lb-exif", ".lb-pos", ".lb-stage"]) { lb.q[k] = el("div"); }
  for (const k of [".lb-pick", ".lb-prev", ".lb-next", ".lb-close"]) { lb.q[k] = el("button"); }
  lb.qa["button"] = [lb.q[".lb-prev"], lb.q[".lb-next"], lb.q[".lb-pick"], lb.q[".lb-close"]];
  lb.hidden = true;
  const bar = el("div");
  const buttons = {};
  for (const a of ["copy", "txt", "json", "clear"]) {
    buttons[a] = el("button", { "data-act": a, "data-needs-picks": "1" });
  }
  Object.assign(doc, {
    body, activeElement: null,
    documentElement: el("html", { dir: c.rtl ? "rtl" : "ltr" }),
    keydown: [],
    addEventListener: (t, f) => { if (t === "keydown") { doc.keydown.push(f); } },
    createElement: (t) => el(t),
    execCommand: () => false,
    querySelector: (s) => ({ ".picks-count": doc.count, ".picks-note": doc.note,
      ".picks-list": doc.list, ".lightbox": lb, ".picks-bar": bar })[s] || null,
    querySelectorAll: (s) => ({ ".grid .tile": tiles, "[data-needs-picks]": Object.values(buttons) })[s] || [],
    count: el("span"), note: el("p"), list: el("textarea"),
  });
  const window = {
    localStorage: { getItem: (k) => (k in store ? store[k] : null), setItem: (k, v) => { store[k] = String(v); } },
    isSecureContext: false,
    confirm: () => true,
  };
  const ctx = {
    document: doc, window, navigator: {}, setTimeout: (f) => f(), JSON, Array, String, Math,
    Image: function () {},
    Blob: function (parts) { this.text = parts.join(""); },
    URL: { createObjectURL: (b) => { const u = "blob:" + nblob++; blobs[u] = b.text; return u; },
      revokeObjectURL: () => {} },
  };
  vm.runInNewContext(code, ctx);
  return { tiles, lb, bar, buttons };
}
function key(k) { const ev = { key: k, preventDefault() {} }; doc.keydown.forEach((f) => f(ev)); }

let p = page();
for (const i of c.pick) { p.tiles[i].q[".pick"].fire("click", click(p.tiles[i].q[".pick"])); }
const countAfterPicks = doc.count.textContent;
p.bar.fire("click", click(p.buttons.txt));
p.bar.fire("click", click(p.buttons.json));
// Reload: the same storage, a fresh page.
p = page();
const pressed = p.tiles.map((t) => t.q[".pick"].getAttribute("aria-pressed") === "true");
// The lightbox: open the first photo, step on, pick it with P, step back past the start.
p.tiles[0].q[".open"].fire("click", click(p.tiles[0].q[".open"]));
const opened = !p.lb.hidden;
key("ArrowRight");
const second = p.lb.q[".lb-pos"].textContent;
key("p");
key("ArrowLeft");
key("ArrowLeft");
const wrapped = p.lb.q[".lb-pos"].textContent;
key("Escape");
console.log(JSON.stringify({
  count: countAfterPicks, txt: downloads["picks.txt"], json: downloads["picks.json"],
  stored: store["specular-picks:" + c.id], pressed, opened, second, wrapped,
  closed: p.lb.hidden, after: doc.count.textContent,
}));
