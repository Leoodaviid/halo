// SPDX-License-Identifier: GPL-2.0-or-later
//
// Janela de preferências do halo: a fonte da temperatura na barra. Roda em
// outro processo, sem o estado do Shell, então lê o halod pelo próprio proxy,
// sem ativá-lo (spec gnome-panel, "Painel não inicia o daemon sozinho").

import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import Gtk from 'gi://Gtk';

import {ExtensionPreferences} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

import {COOLANT_HINT, cpuOption} from './lib/prefs.js';
import {COOLANT, CPU} from './lib/temperature.js';

const BUS_NAME = 'io.github.leoodaviid.Halo1';
const WATERFORCE_PATH = '/io/github/leoodaviid/Halo1/waterforce';
const TELEMETRY = 'io.github.leoodaviid.Halo1.Waterforce.Telemetry';
const KEY = 'temperature-source';

// O halod publica a CPU: tem dono e a propriedade em cache. Um halod antigo,
// sem a propriedade, conta como não publicado.
function cpuPublished(proxy) {
    return proxy !== null && proxy.g_name_owner !== null &&
        proxy.get_cached_property('CpuMilliCelsius') !== null;
}

export default class HaloPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const settings = this.getSettings();
        window.title = 'Preferências do halo';
        window.set_default_size(400, 320);

        const page = new Adw.PreferencesPage();
        const group = new Adw.PreferencesGroup({title: 'Temperatura na barra'});
        page.add(group);
        window.add(page);

        const coolantCheck = new Gtk.CheckButton({valign: Gtk.Align.CENTER});
        const cpuCheck = new Gtk.CheckButton({valign: Gtk.Align.CENTER, group: coolantCheck});
        const coolantRow = new Adw.ActionRow({
            title: 'Líquido do Waterforce', subtitle: COOLANT_HINT, activatable_widget: coolantCheck,
        });
        const cpuRow = new Adw.ActionRow({title: 'CPU', activatable_widget: cpuCheck});
        coolantRow.add_prefix(coolantCheck);
        cpuRow.add_prefix(cpuCheck);
        group.add(coolantRow);
        group.add(cpuRow);

        let proxy = null;
        const render = () => {
            const source = settings.get_string(KEY);
            const checked = source === CPU ? cpuCheck : coolantCheck;
            if (!checked.active)
                checked.active = true;
            const view = cpuOption(cpuPublished(proxy), source);
            cpuRow.sensitive = view.sensitive;
            cpuRow.subtitle = view.hint;
        };

        coolantCheck.connect('toggled', () => {
            if (coolantCheck.active && settings.get_string(KEY) !== COOLANT)
                settings.set_string(KEY, COOLANT);
        });
        cpuCheck.connect('toggled', () => {
            if (cpuCheck.active && settings.get_string(KEY) !== CPU)
                settings.set_string(KEY, CPU);
        });
        const settingsChanged = settings.connect(`changed::${KEY}`, () => render());

        // O dono e as propriedades mudam com a janela aberta: o GDBusProxy
        // relê as propriedades quando o halod aparece.
        const cancellable = new Gio.Cancellable();
        const proxySignals = [];
        Gio.DBusProxy.new_for_bus(Gio.BusType.SESSION, Gio.DBusProxyFlags.DO_NOT_AUTO_START,
            null, BUS_NAME, WATERFORCE_PATH, TELEMETRY, cancellable, (_source, result) => {
                try {
                    proxy = Gio.DBusProxy.new_for_bus_finish(result);
                } catch (e) {
                    if (!cancellable.is_cancelled())
                        console.warn(`halo: proxy da telemetria nas preferências: ${e.message}`);
                    return;
                }
                proxySignals.push(
                    proxy.connect('notify::g-name-owner', () => render()),
                    proxy.connect('g-properties-changed', () => render()));
                render();
            });

        window.connect('close-request', () => {
            cancellable.cancel();
            for (const id of proxySignals)
                proxy?.disconnect(id);
            proxy = null;
            settings.disconnect(settingsChanged);
            return false;
        });

        render();
    }
}
