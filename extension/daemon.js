// SPDX-License-Identifier: GPL-2.0-or-later
//
// Cliente do halod e do systemd do usuário. Junta tudo num estado único e
// avisa a cada mudança; quem desenha é o indicador. Nenhum proxy ativa o
// halod: o painel só observa o nome, e iniciar o daemon é sempre um gesto do
// usuário (spec gnome-panel, "Painel não inicia o daemon sozinho").

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {journalLines} from './lib/journal.js';
import {countPresent, devicesFrom, lastAppliedFrom} from './lib/view.js';

Gio._promisify(Gio.DBusConnection.prototype, 'call');
Gio._promisify(Gio.Subprocess.prototype, 'communicate_utf8_async');

const BUS_NAME = 'io.github.leoodaviid.Halo1';
const ROOT_PATH = '/io/github/leoodaviid/Halo1';
const WATERFORCE_PATH = `${ROOT_PATH}/waterforce`;
const UNIT = 'halod.service';

const ManagerProxy = Gio.DBusProxy.makeProxyWrapper(`
<node>
  <interface name="io.github.leoodaviid.Halo1.Manager">
    <method name="Devices"><arg type="aa{sv}" direction="out"/></method>
    <signal name="DevicesChanged"><arg type="aa{sv}"/></signal>
  </interface>
</node>`);

const TelemetryProxy = Gio.DBusProxy.makeProxyWrapper(`
<node>
  <interface name="io.github.leoodaviid.Halo1.Waterforce.Telemetry">
    <property name="Available" type="b" access="read"/>
    <property name="CoolantMilliCelsius" type="i" access="read"/>
    <property name="CoolantValid" type="b" access="read"/>
  </interface>
</node>`);

// Só a leitura do último LED aplicado: o Apply não está na interface, então
// não há como chamá-lo por engano daqui.
const LedProxy = Gio.DBusProxy.makeProxyWrapper(`
<node>
  <interface name="io.github.leoodaviid.Halo1.Waterforce.Led">
    <property name="LastApplied" type="a{sv}" access="read"/>
  </interface>
</node>`);

const NO_START = Gio.DBusProxyFlags.DO_NOT_AUTO_START;

function cancelled(e) {
    return e instanceof GLib.Error &&
        e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED);
}

// Um a{sv} já desempacotado um nível tem os valores ainda em GLib.Variant.
function unpackDict(dict) {
    return Object.fromEntries(Object.entries(dict ?? {}).map(
        ([key, value]) => [key, value instanceof GLib.Variant ? value.recursiveUnpack() : value]));
}

function newProxy(Wrapper, path, cancellable) {
    return new Promise((resolve, reject) => {
        // eslint-disable-next-line no-new
        new Wrapper(Gio.DBus.session, BUS_NAME, path, (proxy, error) => {
            if (error)
                reject(error);
            else
                resolve(proxy);
        }, cancellable, NO_START);
    });
}

export class HaloDaemon {
    // onChange(state) a cada mudança do estado.
    constructor(initialState, onChange) {
        this._state = {...initialState};
        this._onChange = onChange;
        this._cancellable = new Gio.Cancellable();
        this._signals = [];
        this._devicesSignal = null;
        this._manager = null;
        this._telemetry = null;
        this._led = null;
    }

    get state() {
        return this._state;
    }

    async start() {
        try {
            [this._manager, this._telemetry, this._led] = await Promise.all([
                newProxy(ManagerProxy, ROOT_PATH, this._cancellable),
                newProxy(TelemetryProxy, WATERFORCE_PATH, this._cancellable),
                newProxy(LedProxy, WATERFORCE_PATH, this._cancellable),
            ]);
        } catch (e) {
            if (!cancelled(e))
                console.warn(`halo: não consegui criar os proxies do halod: ${e.message}`);
            return;
        }
        this._connect(this._telemetry, 'notify::g-name-owner', () => this._ownerChanged());
        this._connect(this._telemetry, 'g-properties-changed', () => this._readTelemetry());
        this._connect(this._led, 'g-properties-changed', () => this._readLed());
        this._devicesSignal = this._manager.connectSignal('DevicesChanged',
            (_proxy, _sender, [devices]) => this._setDevices(devices));
        this._ownerChanged();
    }

    destroy() {
        this._cancellable.cancel();
        for (const [object, id] of this._signals)
            object.disconnect(id);
        this._signals = [];
        if (this._devicesSignal !== null)
            this._manager?.disconnectSignal(this._devicesSignal);
        this._devicesSignal = null;
        this._manager = null;
        this._telemetry = null;
        this._led = null;
        this._onChange = null;
    }

    _connect(object, signal, callback) {
        this._signals.push([object, object.connect(signal, callback)]);
    }

    _set(changes) {
        this._state = {...this._state, ...changes};
        this._onChange?.(this._state);
    }

    _ownerChanged() {
        const owner = this._telemetry?.g_name_owner !== null &&
            this._telemetry?.g_name_owner !== undefined;
        this._set({owner, journal: null});
        if (owner) {
            this._readTelemetry();
            this._readLed();
            this._readDevices();
        } else {
            // O cache de um proxy sem dono não vale.
            this._set({devices: [], devicesPresent: 0, lastApplied: null});
            this.refreshUnit();
        }
    }

    _readTelemetry() {
        const t = this._telemetry;
        if (!t)
            return;
        this._set({
            telemetry: {
                available: t.Available ?? false,
                coolantMilliCelsius: t.CoolantMilliCelsius ?? 0,
                coolantValid: t.CoolantValid ?? false,
            },
        });
    }

    _readLed() {
        if (!this._led || !this._state.owner)
            return;
        this._set({lastApplied: lastAppliedFrom(unpackDict(this._led.LastApplied))});
    }

    _setDevices(list) {
        const devices = devicesFrom(list.map(unpackDict));
        this._set({devices, devicesPresent: countPresent(devices)});
    }

    async _readDevices() {
        try {
            const [devices] = await this._manager.DevicesAsync();
            this._setDevices(devices);
        } catch (e) {
            if (!cancelled(e))
                console.warn(`halo: Devices falhou: ${e.message}`);
        }
    }

    async _systemd(method, args, type) {
        return Gio.DBus.session.call(
            'org.freedesktop.systemd1', '/org/freedesktop/systemd1',
            'org.freedesktop.systemd1.Manager', method, args, type,
            Gio.DBusCallFlags.NO_AUTO_START, -1, this._cancellable);
    }

    // 'loaded', 'not-found' ou 'error', pelo LoadState da unidade.
    async refreshUnit() {
        try {
            const reply = await this._systemd('LoadUnit',
                new GLib.Variant('(s)', [UNIT]), new GLib.VariantType('(o)'));
            const [path] = reply.deepUnpack();
            const prop = await Gio.DBus.session.call(
                'org.freedesktop.systemd1', path, 'org.freedesktop.DBus.Properties', 'Get',
                new GLib.Variant('(ss)', ['org.freedesktop.systemd1.Unit', 'LoadState']),
                new GLib.VariantType('(v)'), Gio.DBusCallFlags.NO_AUTO_START, -1,
                this._cancellable);
            const loadState = prop.recursiveUnpack()[0];
            this._set({unit: loadState === 'not-found' ? 'not-found' : 'loaded'});
        } catch (e) {
            if (!cancelled(e))
                this._set({unit: 'error'});
        }
    }

    // Limpa o estado de falha (sem a unidade em falha, o systemd recusa, e
    // isso não importa) e pede o início. O painel muda quando o nome aparecer.
    async startService() {
        try {
            await this._systemd('ResetFailedUnit', new GLib.Variant('(s)', [UNIT]), null);
        } catch (e) {
            if (cancelled(e))
                return;
        }
        try {
            await this._systemd('StartUnit',
                new GLib.Variant('(ss)', [UNIT, 'replace']), new GLib.VariantType('(o)'));
        } catch (e) {
            if (!cancelled(e))
                console.warn(`halo: não consegui iniciar o ${UNIT}: ${e.message}`);
        }
    }

    async showJournal() {
        try {
            const proc = Gio.Subprocess.new(
                ['journalctl', '--user', '-u', UNIT, '-n', '20', '--no-pager', '-o', 'short-iso'],
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_MERGE);
            const [stdout] = await proc.communicate_utf8_async(null, this._cancellable);
            this._set({journal: journalLines(stdout)});
        } catch (e) {
            if (!cancelled(e))
                this._set({journal: journalLines(`não consegui ler o registro: ${e.message}`)});
        }
    }

    hideJournal() {
        this._set({journal: null});
    }
}
