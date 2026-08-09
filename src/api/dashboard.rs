use axum::response::Html;

pub async fn dashboard_handler() -> Html<String> {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Compute Chain - Control Panel</title>
    <style>
        /* ============ CSS Variables ============ */
        :root {
            --bg: #0a0e14;
            --surface: #141b22;
            --border: #1e2a36;
            --text: #c9d1d9;
            --text-dim: #8b949e;
            --accent: #58a6ff;
            --green: #3fb950;
            --yellow: #d2991d;
            --red: #f85149;
            --purple: #a371f7;
            --radius: 8px;
            --gap: 12px;
        }
        
        * { margin: 0; padding: 0; box-sizing: border-box; }
        
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, monospace;
            background: var(--bg);
            color: var(--text);
            min-height: 100vh;
            line-height: 1.5;
        }
        
        .container { max-width: 1400px; margin: 0 auto; padding: 16px; }
        
        header {
            display: flex; justify-content: space-between; align-items: center;
            padding: 12px 0; border-bottom: 1px solid var(--border); margin-bottom: 16px;
        }
        
        header h1 { font-size: 1.3em; color: var(--accent); }
        header .status { display: flex; gap: 16px; font-size: 0.85em; }
        .status-dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; margin-right: 4px; }
        .status-dot.online { background: var(--green); }
        .status-dot.warning { background: var(--yellow); }
        .status-dot.offline { background: var(--red); }
        
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: var(--gap); }
        .grid-2 { grid-template-columns: 1fr 1fr; }
        .grid-3 { grid-template-columns: repeat(3, 1fr); }
        
        .card {
            background: var(--surface); border: 1px solid var(--border);
            border-radius: var(--radius); padding: 14px;
        }
        .card h2 { font-size: 0.9em; text-transform: uppercase; letter-spacing: 1px; color: var(--text-dim); margin-bottom: 10px; }
        .card .value { font-size: 2em; font-weight: bold; }
        .card .sub { font-size: 0.75em; color: var(--text-dim); }
        
        .metric-row { display: flex; justify-content: space-between; padding: 4px 0; border-bottom: 1px solid var(--border); }
        .metric-row:last-child { border: none; }
        .metric-label { color: var(--text-dim); }
        .metric-value { font-weight: bold; }
        
        table { width: 100%; border-collapse: collapse; font-size: 0.85em; }
        th { text-align: left; color: var(--text-dim); padding: 6px 8px; border-bottom: 2px solid var(--border); }
        td { padding: 6px 8px; border-bottom: 1px solid var(--border); }
        tr:hover { background: rgba(88, 166, 255, 0.05); }
        
        .badge {
            display: inline-block; padding: 2px 8px; border-radius: 12px;
            font-size: 0.75em; font-weight: bold;
        }
        .badge.green { background: rgba(63, 185, 80, 0.2); color: var(--green); }
        .badge.yellow { background: rgba(210, 153, 29, 0.2); color: var(--yellow); }
        .badge.red { background: rgba(248, 81, 73, 0.2); color: var(--red); }
        .badge.purple { background: rgba(163, 113, 247, 0.2); color: var(--purple); }
        
        button {
            background: var(--accent); color: #000; border: none;
            padding: 8px 16px; border-radius: 6px; cursor: pointer;
            font-weight: bold; font-size: 0.85em; margin: 2px;
            transition: opacity 0.2s;
        }
        button:hover { opacity: 0.85; }
        button.danger { background: var(--red); color: #fff; }
        button.outline { background: transparent; border: 1px solid var(--border); color: var(--text); }
        
        .pbft-flow { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; padding: 8px 0; }
        .pbft-step {
            padding: 8px 14px; border-radius: var(--radius); border: 1px solid var(--border);
            text-align: center; font-size: 0.8em; transition: all 0.3s;
        }
        .pbft-step.active { border-color: var(--green); background: rgba(63, 185, 80, 0.1); }
        .pbft-arrow { color: var(--text-dim); font-size: 1.2em; }
        
        .log-container {
            max-height: 300px; overflow-y: auto; font-size: 0.78em;
            font-family: 'Courier New', monospace;
        }
        .log-entry { padding: 2px 0; border-bottom: 1px solid rgba(255,255,255,0.03); }
        .log-time { color: var(--text-dim); margin-right: 8px; }
        .log-p2p { color: var(--accent); }
        .log-block { color: var(--green); }
        .log-pbft { color: var(--purple); }
        .log-error { color: var(--red); }
        
        .tabs { display: flex; gap: 4px; margin-bottom: 10px; }
        .tab {
            padding: 6px 14px; border-radius: 6px 6px 0 0; cursor: pointer;
            background: transparent; border: 1px solid transparent; color: var(--text-dim);
            font-size: 0.85em;
        }
        .tab.active { background: var(--surface); border-color: var(--border); color: var(--text); }
        
        @media (max-width: 768px) {
            .grid-2, .grid-3 { grid-template-columns: 1fr; }
            header { flex-direction: column; gap: 8px; }
        }
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🚀 Compute Chain Control Panel</h1>
            <div class="status">
                <span><span class="status-dot online" id="status-dot"></span><span id="status-text">Connected</span></span>
                <span>⏱ <span id="uptime">--</span></span>
                <span>API: <span id="api-port">3000</span></span>
            </div>
        </header>
        
        <!-- 📊 Overview -->
        <div class="grid grid-3" style="margin-bottom: var(--gap);">
            <div class="card">
                <h2>📦 Blockchain</h2>
                <div class="value" id="ov-height">--</div>
                <div class="sub">Height | Last: <span id="ov-last-hash">--</span></div>
                <div class="sub">TPS: <span id="ov-tps">--</span> | Block time: <span id="ov-block-time">--</span></div>
            </div>
            <div class="card">
                <h2>👥 Network</h2>
                <div class="value" id="ov-peers">--</div>
                <div class="sub">Peers | Status: <span id="ov-net-status">--</span></div>
                <div class="sub">Avg Ping: <span id="ov-avg-ping">--</span> ms</div>
            </div>
            <div class="card">
                <h2>🗳️ Consensus</h2>
                <div class="value" id="ov-leader">--</div>
                <div class="sub">Leader | Round: <span id="ov-round">--</span> | View: <span id="ov-view">--</span></div>
                <div class="sub">Votes: <span id="ov-votes">--</span></div>
            </div>
        </div>
        
        <!-- ⚙️ Network Health -->
        <div class="card" style="margin-bottom: var(--gap);">
            <h2>⚙️ Network Diagnostics</h2>
            <div class="grid grid-3">
                <div>
                    <div class="metric-row"><span class="metric-label">Connected Peers</span><span class="metric-value" id="nh-peers">--</span></div>
                    <div class="metric-row"><span class="metric-label">Kademlia Status</span><span class="metric-value" id="nh-kad">--</span></div>
                    <div class="metric-row"><span class="metric-label">Gossipsub Status</span><span class="metric-value" id="nh-gossip">--</span></div>
                </div>
                <div>
                    <div class="metric-row"><span class="metric-label">Last Sync</span><span class="metric-value" id="nh-last-sync">--</span></div>
                    <div class="metric-row"><span class="metric-label">Dropped Messages</span><span class="metric-value" id="nh-dropped">0</span></div>
                    <div class="metric-row"><span class="metric-label">Avg Ping</span><span class="metric-value" id="nh-avg-ping">-- ms</span></div>
                </div>
                <div>
                    <button onclick="syncChain()">🔄 Sync Chain</button>
                    <button onclick="mineBlock()">⛏ Mine Block</button>
                    <button onclick="refreshAll()">🔄 Refresh</button>
                </div>
            </div>
        </div>
        
        <!-- 🗳️ PBFT Flow -->
        <div class="card" style="margin-bottom: var(--gap);">
            <h2>🗳️ PBFT Consensus Flow</h2>
            <div class="pbft-flow">
                <div class="pbft-step" id="pbft-leader">👑 Leader</div>
                <span class="pbft-arrow">→</span>
                <div class="pbft-step" id="pbft-prepare">📋 PrePrepare</div>
                <span class="pbft-arrow">→</span>
                <div class="pbft-step" id="pbft-vote">📝 Prepare</div>
                <span class="pbft-arrow">→</span>
                <div class="pbft-step" id="pbft-commit">✅ Commit</div>
                <span class="pbft-arrow">→</span>
                <div class="pbft-step" id="pbft-finalized">🏁 Finalized</div>
            </div>
            <div style="font-size:0.8em; color:var(--text-dim); margin-top:8px;" id="pbft-info">
                Leader: <span id="pbft-leader-name">--</span> | 
                Round: <span id="pbft-round">--</span> | 
                View: <span id="pbft-view">--</span> |
                PrePrepare: <span id="pbft-count-pp">0</span> |
                Prepare: <span id="pbft-count-p">0</span> |
                Commit: <span id="pbft-count-c">0</span>
            </div>
        </div>
        
        <!-- 📜 Event Log with Tabs -->
        <div class="card">
            <h2>📜 Event Log</h2>
            <div class="tabs">
                <div class="tab active" onclick="filterLog('all')">All</div>
                <div class="tab" onclick="filterLog('p2p')">P2P</div>
                <div class="tab" onclick="filterLog('block')">Blockchain</div>
                <div class="tab" onclick="filterLog('pbft')">PBFT</div>
                <div class="tab" onclick="filterLog('tx')">Transactions</div>
                <div class="tab" onclick="filterLog('error')">Errors</div>
            </div>
            <div class="log-container" id="log-container">
                <div class="log-entry"><span class="log-time">--:--:--</span>📡 Dashboard ready...</div>
            </div>
        </div>
    </div>
    
    <script>
        const API = window.location.origin;
        let logEntries = [];
        let logFilter = 'all';
        
        function addLog(type, msg) {
            const now = new Date().toLocaleTimeString();
            logEntries.unshift({ time: now, type, msg });
            if (logEntries.length > 500) logEntries.pop();
            renderLog();
        }
        
        function filterLog(type) {
            logFilter = type;
            document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
            event.target.classList.add('active');
            renderLog();
        }
        
        function renderLog() {
            const container = document.getElementById('log-container');
            const filtered = logFilter === 'all' ? logEntries : logEntries.filter(e => e.type === logFilter);
            container.innerHTML = filtered.map(e => 
                `<div class="log-entry"><span class="log-time">${e.time}</span><span class="log-${e.type}">${e.msg}</span></div>`
            ).join('');
        }
        
        async function fetchJSON(url) {
            try { const res = await fetch(API + url); return await res.json(); }
            catch(e) { return null; }
        }
        
        async function refreshAll() {
            // Chain info
            const chain = await fetchJSON('/chain');
            if (chain) {
                document.getElementById('ov-height').textContent = chain.height || 0;
                document.getElementById('ov-last-hash').textContent = (chain.last_block_hash || '').substring(0, 14) + '...';
                document.getElementById('pbft-leader-name').textContent = chain.pbft_leader || '--';
                document.getElementById('pbft-round').textContent = chain.pbft_round || 0;
                document.getElementById('pbft-view').textContent = chain.pbft_view || 0;
                document.getElementById('ov-leader').textContent = chain.pbft_leader || '--';
                document.getElementById('ov-round').textContent = chain.pbft_round || 0;
                document.getElementById('ov-view').textContent = chain.pbft_view || 0;
            }
            
            // Peers
            const peers = await fetchJSON('/p2p/peers');
            if (peers) {
                document.getElementById('ov-peers').textContent = peers.count || 0;
                document.getElementById('nh-peers').textContent = peers.count || 0;
                document.getElementById('ov-net-status').textContent = peers.count > 0 ? 'Connected' : 'Standalone';
                const dot = document.getElementById('status-dot');
                dot.className = 'status-dot ' + (peers.count > 0 ? 'online' : 'warning');
            }
            
            // Health
            const health = await fetchJSON('/health');
            if (health) {
                document.getElementById('nh-kad').textContent = health.p2p ? 'OK' : 'Error';
                document.getElementById('nh-gossip').textContent = health.p2p ? 'OK' : 'Error';
            }
            
            document.getElementById('uptime').textContent = new Date().toLocaleTimeString();
        }
        
        async function mineBlock() {
            addLog('block', '⛏ Mining block...');
            const result = await fetch(API + '/block/mine', {
                method: 'POST', headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({validator_id:'node1', program:[{opcode:'MOV',params:[0,99]},{opcode:'HALT',params:[]}]})
            });
            const data = await result.json();
            if (data.status === 'block_mined') {
                addLog('block', '✅ Block mined: height=' + data.block_height + ' hash=' + data.block_hash.substring(0,14));
                refreshAll();
            } else {
                addLog('error', '❌ Mine failed: ' + (data.message || data.error || 'unknown'));
            }
        }
        
        async function syncChain() {
            addLog('p2p', '🔄 Requesting chain sync...');
            const result = await fetchJSON('/chain/sync');
            if (result) {
                addLog('p2p', '✅ Chain synced: ' + (result.blocks?.length || 0) + ' blocks');
            }
            refreshAll();
        }
        
        // WebSocket for live updates
        const wsUrl = window.location.origin.replace('http', 'ws').replace(':3000',':9000').replace(':3001',':9001').replace(':3002',':9002');
        try {
            const ws = new WebSocket(wsUrl);
            ws.onopen = () => addLog('p2p', '🔌 WebSocket connected');
            ws.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    if (data.type === 'new_block') {
                        addLog('block', '📦 New block: height=' + data.height + ' hash=' + (data.hash||'').substring(0,14));
                    }
                    refreshAll();
                } catch(e) {}
            };
            ws.onclose = () => addLog('error', '🔌 WebSocket disconnected');
            ws.onerror = () => addLog('error', '⚠️ WebSocket error');
        } catch(e) {
            addLog('error', '⚠️ WebSocket unavailable');
        }
        
        // Initial load + periodic refresh
        refreshAll();
        setInterval(refreshAll, 5000);
        addLog('p2p', '📡 Dashboard initialized');
    </script>
</body>
</html>"#;

    Html(html.to_string())
}

// Strategy: Insert Mediator for api (Interface)
// Review and adjust before applying.
