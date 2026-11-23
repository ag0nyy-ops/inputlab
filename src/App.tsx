// inputlab · © 2026 Jay · https://github.com/ag0nyy-ops/inputlab
import { useEffect } from 'react';
import { Eye } from './components/Eye';
import { Stamp } from './components/Stamp';
import { getBridge } from './lib/bridge';
import { initEyes } from './lib/eye';
import { setState, useStore } from './lib/store';
import { backendLabel } from './lib/format';
import { DeviceList } from './views/DeviceList';
import { DevicePanel } from './views/DevicePanel';
import { SessionScreen } from './views/SessionScreen';
import { SettingsView } from './views/SettingsView';
import { SummaryView } from './views/SummaryView';

export function App() {
  const info = useStore((s) => s.info);
  const screen = useStore((s) => s.screen);

  useEffect(() => {
    let off = () => {};
    let alive = true;
    void getBridge().then(async (b) => {
      const i = await b.appInfo();
      if (!alive) return;
      setState({ info: i });
      off = b.subscribeDevices((m) => setState({ devices: m.devices, devicesLoaded: true }));
    });
    const stopEyes = initEyes();
    return () => {
      alive = false;
      off();
      stopEyes();
    };
  }, []);

  if (screen === 'session') return <SessionScreen />;

  return (
    <div className="app">
      <header className="top">
        <Eye className="eye--logo" blink={5.9} delay={-2.4} />
        <span className="top__name">inputlab</span>
        <span className="top__sp" />
        {info && (
          <div className="top__meta">
            <span className="label label--sm">{backendLabel(info.rawBackend)}</span>
            {info.noCapture && <Stamp tone="wip" size="sm">no capture</Stamp>}
          </div>
        )}
        <button className="btn btn--sm" onClick={() => setState({ screen: screen === 'settings' ? 'home' : 'settings' })} aria-pressed={screen === 'settings'}>
          settings
        </button>
      </header>
      <div className="shell">
        <DeviceList />
        <main id="main" className="main scroll">
          {screen === 'settings' ? <SettingsView /> : screen === 'summary' ? <SummaryView /> : <DevicePanel />}
        </main>
      </div>
    </div>
  );
}
