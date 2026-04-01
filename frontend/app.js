const $ = (id) => document.getElementById(id);

function log(msg) {
  const ts = new Date().toLocaleTimeString();
  $("log").textContent = `[${ts}] ${msg}\n` + $("log").textContent;
}

async function api(path, opts = {}) {
  const r = await fetch(path, opts);
  if (!r.ok) throw new Error(`${path} -> HTTP ${r.status}`);
  return r.json();
}

function fillToySelect(id, devices, selectedIndex) {
  const el = $(id);
  el.innerHTML = "";

  // no auto option by design
  for (const d of devices) {
    const o = document.createElement("option");
    o.value = String(d.index);
    o.textContent = `${d.name} (#${d.index})`;
    el.appendChild(o);
  }

  if (devices.length === 0) {
    const o = document.createElement("option");
    o.value = "";
    o.textContent = "(No devices connected)";
    el.appendChild(o);
    el.value = "";
    return;
  }

  if (selectedIndex == null) {
    el.value = ""; // force explicit user selection
  } else {
    el.value = String(selectedIndex);
    if (el.value !== String(selectedIndex)) {
      el.value = "";
    }
  }
}

function setReaction(prefix, r) {
  $(`${prefix}_enabled`).checked = !!r.enabled;
  $(`${prefix}_mode`).value = r.mode;
  $(`${prefix}_intensity`).value = r.intensity;
  $(`${prefix}_intensity_val`).textContent = Number(r.intensity).toFixed(2);
  $(`${prefix}_duration_ms`).value = r.duration_ms;
  $(`${prefix}_cooldown_ms`).value = r.cooldown_ms;
  $(`${prefix}_toy`).value = r.toy_index == null ? "" : String(r.toy_index);
}

function getReaction(prefix) {
  const toyEl = $(`${prefix}_toy`);
  const toyVal = toyEl.value;
  const toyIndex = toyVal === "" ? null : Number(toyVal);

  let toyName = null;
  if (toyVal !== "") {
    const txt = toyEl.options[toyEl.selectedIndex]?.textContent || "";
    toyName = txt.replace(/\s+\(#\d+\)$/, "");
  }

  return {
    enabled: $(`${prefix}_enabled`).checked,
    toy_name: toyName,
    toy_index: toyIndex,
    mode: $(`${prefix}_mode`).value,
    intensity: Number($(`${prefix}_intensity`).value),
    duration_ms: Number($(`${prefix}_duration_ms`).value),
    cooldown_ms: Number($(`${prefix}_cooldown_ms`).value),
  };
}

function validateRequiredToySelection() {
  const hitEnabled = $("hit_enabled").checked;
  const missEnabled = $("miss_enabled").checked;

  if (hitEnabled && $("hit_toy").value === "") {
    throw new Error("Hit reaction is enabled but no toy is selected.");
  }
  if (missEnabled && $("miss_toy").value === "") {
    throw new Error("Miss reaction is enabled but no toy is selected.");
  }
}

async function refreshAll() {
  const [status, cfg, devices] = await Promise.all([
    api("/api/status"),
    api("/api/config"),
    api("/api/devices"),
  ]);

  $("status").textContent = JSON.stringify(status, null, 2);
  $("datapuller_url").value = cfg.datapuller_url || "";

  fillToySelect("hit_toy", devices, cfg.reactions?.hit?.toy_index);
  fillToySelect("miss_toy", devices, cfg.reactions?.miss?.toy_index);

  setReaction("hit", cfg.reactions.hit);
  setReaction("miss", cfg.reactions.miss);

  log(`Loaded config. Devices: ${devices.length}`);
}

async function saveConfig() {
  validateRequiredToySelection();

  const body = {
    datapuller_url: $("datapuller_url").value.trim(),
    reactions: {
      hit: getReaction("hit"),
      miss: getReaction("miss"),
    },
  };

  const res = await api("/api/config", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });

  log(`Save: ${JSON.stringify(res)}`);
  await refreshAll();
}

async function testReaction(name) {
  const res = await api(`/api/test?reaction=${encodeURIComponent(name)}`, {
    method: "POST",
  });
  log(`Test ${name}: ${JSON.stringify(res)}`);
}

["hit", "miss"].forEach((p) => {
  $(`${p}_intensity`).addEventListener("input", (e) => {
    $(`${p}_intensity_val`).textContent = Number(e.target.value).toFixed(2);
  });
});

$("save").addEventListener("click", () =>
  saveConfig().catch((e) => log(`Save error: ${e.message}`))
);

$("refresh").addEventListener("click", () =>
  refreshAll().catch((e) => log(`Refresh error: ${e.message}`))
);

$("test_hit").addEventListener("click", () =>
  testReaction("hit").catch((e) => log(`Test hit error: ${e.message}`))
);

$("test_miss").addEventListener("click", () =>
  testReaction("miss").catch((e) => log(`Test miss error: ${e.message}`))
);

refreshAll().catch((e) => log(`Init error: ${e.message}`));
