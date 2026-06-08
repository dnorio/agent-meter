// agent-meter shared js
(function(){
  const root = document.documentElement;
  const stored = localStorage.getItem('am-theme');
  if (stored) root.setAttribute('data-theme', stored);
  window.amToggleTheme = function(){
    const cur = root.getAttribute('data-theme') || 'dark';
    const next = cur === 'dark' ? 'light' : 'dark';
    root.setAttribute('data-theme', next);
    localStorage.setItem('am-theme', next);
  };

  // copy buttons
  document.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-copy]');
    if (!btn) return;
    e.preventDefault();
    const txt = btn.getAttribute('data-copy');
    navigator.clipboard.writeText(txt).then(() => {
      const old = btn.textContent;
      btn.textContent = 'Copied!';
      setTimeout(() => btn.textContent = old, 1200);
    });
  });

  // sidebar collapse
  window.amToggleSidebar = function(){
    const app = document.querySelector('.am-app');
    if (!app) return;
    const collapsed = app.getAttribute('data-collapsed') === 'true';
    app.setAttribute('data-collapsed', collapsed ? 'false' : 'true');
    localStorage.setItem('am-sidebar-collapsed', collapsed ? 'false' : 'true');
  };

  document.addEventListener('DOMContentLoaded', () => {
    const app = document.querySelector('.am-app');
    if (app && localStorage.getItem('am-sidebar-collapsed') === 'true') {
      app.setAttribute('data-collapsed', 'true');
    }

    // load /api/me into user menu
    fetch('/api/me', {credentials:'include'}).then(r => r.ok ? r.json() : null).then(me => {
      const slot = document.getElementById('amUserSlot');
      if (!slot) return;
      if (me) {
        const name = me.display_name || me.github_login || me.email;
        slot.innerHTML = `
          <a class="am-btn am-btn-ghost am-btn-sm" href="/auth/logout" title="Sign out">
            ${me.avatar_url ? `<img src="${me.avatar_url}" alt="" style="width:20px;height:20px;border-radius:50%">` : '<svg class="am-icon"><use href="/_static/icons.svg#i-user"/></svg>'}
            <span>${name}</span>
          </a>`;
      } else {
        slot.innerHTML = `<a class="am-btn am-btn-secondary am-btn-sm" href="/login"><svg class="am-icon"><use href="/_static/icons.svg#i-github"/></svg>Sign in</a>`;
      }
    }).catch(()=>{});
  });

  // sparkline helper: amSpark(values, w, h, color)
  window.amSpark = function(values, w=80, h=22, color='var(--am-accent)'){
    if (!values || !values.length) return '';
    const max = Math.max(...values, 1);
    const min = Math.min(...values, 0);
    const range = (max - min) || 1;
    const step = w / Math.max(values.length - 1, 1);
    const pts = values.map((v, i) => `${i*step},${h - ((v - min)/range) * h}`).join(' ');
    const last = values[values.length-1];
    const lx = (values.length-1) * step;
    const ly = h - ((last - min)/range) * h;
    return `<svg class="am-spark" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none"><polyline points="${pts}" fill="none" stroke="${color}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><circle cx="${lx}" cy="${ly}" r="2" fill="${color}"/></svg>`;
  };

  // delta pill
  window.amDelta = function(curr, prev){
    if (prev == null || prev === 0) return '';
    const d = ((curr - prev) / Math.abs(prev)) * 100;
    const cls = d > 0.5 ? 'up' : d < -0.5 ? 'down' : 'flat';
    const arrow = d > 0.5 ? '▲' : d < -0.5 ? '▼' : '—';
    return `<span class="am-delta ${cls}">${arrow} ${Math.abs(d).toFixed(1)}%</span>`;
  };

  // money formatter
  window.amMoney = function(n){
    if (n == null) return '$0';
    if (n >= 1000) return '$' + n.toFixed(0).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
    if (n >= 10) return '$' + n.toFixed(2);
    if (n >= 1) return '$' + n.toFixed(3);
    return '$' + n.toFixed(4);
  };
  window.amNum = function(n){
    if (n == null) return '0';
    if (n >= 1e6) return (n/1e6).toFixed(1) + 'M';
    if (n >= 1e3) return (n/1e3).toFixed(1) + 'K';
    return Math.round(n).toLocaleString();
  };

  // shared shell renderer — call amShell({active:'cost', title:'Cost', subtitle:'...', breadcrumb:'Cost'})
  window.amShell = function(opts){
    opts = opts || {};
    const active = opts.active || '';

    // Skip-to-content link (a11y T-324.16)
    if (!document.querySelector('.am-skip-link')) {
      const skip = document.createElement('a');
      skip.className = 'am-skip-link';
      skip.href = '#am-main-content';
      skip.textContent = 'Skip to main content';
      document.body.insertBefore(skip, document.body.firstChild);
    }
    // Add id to main element for skip-link target
    const mainEl = document.querySelector('.am-main');
    if (mainEl) { mainEl.id = 'am-main-content'; mainEl.setAttribute('role', 'main'); }

    const navItems = [
      ['dashboard', '/', 'i-dashboard', 'Dashboard'],
      ['conversations', '/conversations', 'i-conversations', 'Conversations'],
      ['cost', '/cost', 'i-cost', 'Cost'],
      ['alerts', '/alerts', 'i-alerts', 'Alerts'],
      ['tasks', '/tasks', 'i-tasks', 'Tasks'],
      ['reports', '/reports', 'i-reports', 'Reports'],
    ];
    const accountItems = [
      ['settings', '/settings', 'i-settings', 'Settings'],
      ['pricing', '/pricing', 'i-pricing', 'Pricing'],
      ['github', 'https://github.com/dnorio/agent-meter', 'i-github', 'GitHub'],
    ];
    const navHtml = navItems.map(([k,h,i,l]) =>
      `<a class="am-nav-link${k===active?' active':''}" href="${h}"><svg class="am-icon"><use href="/_static/icons.svg#${i}"/></svg><span>${l}</span></a>`
    ).join('');
    const accHtml = accountItems.map(([k,h,i,l]) =>
      `<a class="am-nav-link${k===active?' active':''}" href="${h}"><svg class="am-icon"><use href="/_static/icons.svg#${i}"/></svg><span>${l}</span></a>`
    ).join('');
    const sidebar = document.getElementById('amSidebar');
    if (sidebar) {
      sidebar.setAttribute('role', 'navigation');
      sidebar.setAttribute('aria-label', 'Main navigation');
      sidebar.innerHTML = `
        <a href="/" class="am-sidebar-brand">
          <span class="logo-mark"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2.5" stroke-linecap="round"><path d="M5 21V13M12 21v-5M19 21v-9"/><circle cx="5" cy="11" r="1.5" fill="white"/><circle cx="12" cy="14" r="1.5" fill="white"/><circle cx="19" cy="10" r="1.5" fill="white"/></svg></span>
          <span class="logo-text">agent-meter</span>
        </a>
        <nav class="am-sidebar-nav">
          <div class="am-nav-section">Observability</div>
          ${navHtml}
          <div class="am-nav-section">Account</div>
          ${accHtml}
        </nav>
        <div class="am-sidebar-footer">
          <button class="am-btn am-btn-ghost am-btn-icon" onclick="amToggleTheme()" title="Toggle theme"><svg class="am-icon"><use href="/_static/icons.svg#i-sun"/></svg></button>
          <span class="am-status-pill" id="amHealthPill">Live</span>
        </div>`;
    }
    const topbar = document.getElementById('amTopbar');
    if (topbar) {
      topbar.setAttribute('role', 'banner');
      topbar.innerHTML = `
        <div class="am-breadcrumbs">
          <span class="crumb">agent-meter</span>
          <span class="sep">›</span>
          <span class="crumb current">${opts.breadcrumb || opts.title || ''}</span>
        </div>
        <div class="am-search">
          <svg class="am-icon"><use href="/_static/icons.svg#i-search"/></svg>
          <input placeholder="Search…" disabled>
          <span class="kbd">⌘K</span>
        </div>
        <div class="am-topbar-actions" id="amUserSlot">
          <a class="am-btn am-btn-secondary am-btn-sm" href="/login">Sign in</a>
        </div>`;
      // re-fire user-slot fetch
      fetch('/api/me', {credentials:'include'}).then(r => r.ok ? r.json() : null).then(me => {
        const slot = document.getElementById('amUserSlot'); if (!slot) return;
        if (me) {
          const name = me.display_name || me.github_login || me.email;
          slot.innerHTML = `<a class="am-btn am-btn-ghost am-btn-sm" href="/auth/logout" title="Sign out">${me.avatar_url ? `<img src="${me.avatar_url}" alt="" style="width:20px;height:20px;border-radius:50%">` : '<svg class="am-icon"><use href="/_static/icons.svg#i-user"/></svg>'}<span>${name}</span></a>`;
        }
      }).catch(()=>{});
    }
    const footer = document.getElementById('amFooter');
    if (footer) {
      footer.setAttribute('role', 'contentinfo');
      footer.innerHTML = `<span>© 2026 agent-meter</span><a href="/pricing">Pricing</a><a href="https://github.com/dnorio/agent-meter">GitHub</a><span class="am-spacer" style="flex:1"></span><span id="amHealthFoot" class="am-mono am-muted" style="font-size:11px">checking…</span>`;
      fetch('/health').then(r=>r.json()).then(r=>{
        const p = document.getElementById('amHealthPill');
        const f = document.getElementById('amHealthFoot');
        if (p) p.textContent = r.status === 'ok' ? 'Live' : 'Issue';
        if (f) f.textContent = `${r.service} · ${r.status}`;
      }).catch(()=>{});
    }
  };
})();
