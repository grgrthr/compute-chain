use axum::Json;

pub async fn health_handler() -> Json<serde_json::Value> {
    tracing::info!("🏥 Health check requested");
    Json(serde_json::json!({
        "status": "ok",
        "api": true,
        "chain": true,
        "contracts": true,
        "nfts": true,
        "p2p": true,
        "version": "3.4.0"
    }))
}

pub async fn dashboard_handler() -> axum::response::Html<String> {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Compute Chain Dashboard</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: 'Courier New', monospace; background: #0a0a0a; color: #00ff88; padding: 20px; }
        .container { max-width: 900px; margin: 0 auto; }
        h1 { text-align: center; color: #00ff88; margin-bottom: 10px; font-size: 1.5em; }
        .subtitle { text-align: center; color: #008855; margin-bottom: 20px; }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 15px; }
        .card { background: #111; border: 1px solid #00ff8833; border-radius: 8px; padding: 15px; }
        .card h3 { color: #00ff88; font-size: 0.8em; margin-bottom: 8px; text-transform: uppercase; }
        .card .value { font-size: 2em; font-weight: bold; }
        .card .sub { font-size: 0.7em; color: #008855; margin-top: 5px; }
        button { background: #00ff88; color: #000; border: none; padding: 10px 20px; border-radius: 5px; cursor: pointer; font-weight: bold; margin: 5px; }
        button:hover { background: #00cc66; }
        .actions { text-align: center; margin: 20px 0; }
        .log { background: #111; border: 1px solid #00ff8833; border-radius: 8px; padding: 15px; margin-top: 20px; max-height: 200px; overflow-y: auto; font-size: 0.8em; }
        .log p { margin: 3px 0; }
        .refresh { color: #008855; font-size: 0.7em; }
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 Compute Chain Dashboard</h1>
        <p class="subtitle">v1.0 | <span id="time">--</span> | <span class="refresh">WebSocket live</span></p>
        <div class="grid">
            <div class="card"><h3>📦 Blocks</h3><div class="value" id="blocks">--</div><div class="sub" id="last-hash">--</div></div>
            <div class="card"><h3>👥 Peers</h3><div class="value" id="peers">--</div><div class="sub" id="peer-list">--</div></div>
            <div class="card"><h3>⛽ Gas Used</h3><div class="value" id="gas">--</div><div class="sub">fees collected</div></div>
            <div class="card"><h3>💰 Supply</h3><div class="value" id="supply">--</div><div class="sub">total tokens</div></div>
            <div class="card"><h3>🏪 Orders</h3><div class="value" id="orders">--</div><div class="sub">open marketplace</div></div>
            <div class="card"><h3>⛏ Miners</h3><div class="value" id="miners">--</div><div class="sub">registered</div></div>
        </div>
        <div class="actions">
            <button onclick="mineBlock()">⛏ Mine Block</button>
            <button onclick="refreshAll()">🔄 Refresh</button>
        </div>
        <div class="log" id="log"><p>📡 Dashboard ready...</p></div>
    </div>
    <script>
        const API = window.location.origin;
        function log(msg) { const logDiv = document.getElementById('log'); const time = new Date().toLocaleTimeString(); logDiv.innerHTML = `<p>[${time}] ${msg}</p>` + logDiv.innerHTML; if (logDiv.children.length > 20) logDiv.removeChild(logDiv.lastChild); }
        async function fetchJSON(url) { try { const res = await fetch(API + url); return await res.json(); } catch(e) { return null; } }
        async function refreshAll() {
            const chain = await fetchJSON('/chain'); if (chain) { document.getElementById('blocks').textContent = chain.height || 0; document.getElementById('last-hash').textContent = (chain.last_block_hash || '').substring(0, 16) + '...'; }
            const peers = await fetchJSON('/p2p/peers'); if (peers) { document.getElementById('peers').textContent = peers.count || 0; document.getElementById('peer-list').textContent = peers.peers?.join(', ') || 'none'; }
            const gas = await fetchJSON('/gas/stats'); if (gas) { document.getElementById('gas').textContent = (gas.total_gas_used || 0).toLocaleString(); }
            const bal = await fetchJSON('/tx/balance?address=validator1'); if (bal) { document.getElementById('supply').textContent = (bal.total_supply || 0).toLocaleString(); }
            const market = await fetchJSON('/marketplace/stats'); if (market) { document.getElementById('orders').textContent = market.open_orders || 0; }
            const miners = await fetchJSON('/miner/list'); if (miners) { document.getElementById('miners').textContent = miners.length || 0; }
            document.getElementById('time').textContent = new Date().toLocaleTimeString();
        }
        async function mineBlock() {
            log('⛏ Mining block...');
            const result = await fetch(API + '/block/mine', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ validator_id: 'dashboard', program: [{opcode:'MOV',params:[0,99]},{opcode:'HALT',params:[]}] }) });
            const data = await result.json();
            if (data.status === 'block_mined') { log('✅ Block mined! Height: ' + data.block_height); } else { log('❌ ' + (data.error || 'Mine failed')); }
            refreshAll();
        }
        const wsUrl = window.location.origin.replace("http", "ws");
        try { const ws = new WebSocket(wsUrl.replace(":3000", ":9001").replace(":3001", ":9001").replace(":3002", ":9002").replace(":3003", ":9003")); ws.onmessage = (event) => { const data = JSON.parse(event.data); log('⚡ ' + (data.type || 'event')); refreshAll(); }; ws.onopen = () => log('🔌 WebSocket connected'); ws.onclose = () => log('🔌 WebSocket disconnected'); } catch(e) { log('⚠️ WebSocket unavailable'); }
        refreshAll(); setInterval(refreshAll, 3000);
    </script>
</body>
</html>"#;
    axum::response::Html(html.to_string())
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
