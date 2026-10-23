// SPDX-License-Identifier: GPL-2.0-or-later
//
// Carrossel dos aparelhos: o desenho, o nome, o status, a navegação e um
// contêiner `body` para o que vem abaixo de cada aparelho. Só desenha: quem
// decide o aparelho, o passo da roda e o texto do status é lib/carousel.js, e
// o desenho vem pronto de lib/art.js. Medidas e estrutura seguem o protótipo
// (docs/ui/panel-prototype.html: .tile, .stage, .art, .nav, .meta, .dots).

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Gio from 'gi://Gio';
import St from 'gi://St';

import {deviceSvg} from './lib/art.js';
import {
    nextIndex, prevIndex, presentDevices, selectAfterChange, statusText, wheelStep,
} from './lib/carousel.js';

// O SVG é quadrado (300x300, arte de 300x156 no meio). Com 246 px, a arte fica
// em 246x128, como o .art svg do protótipo (280x128 com a proporção mantida);
// o contêiner do desenho (.halo-art-clip, 280x128) corta a sobra transparente.
const ART_SIZE = 246;
// Um clique da roda (UP/DOWN) passa do limiar do protótipo, que mede em pixels;
// a rolagem suave do touchpad vem em fração de clique.
const DISCRETE_DELTA = 100;
const SMOOTH_SCALE = 40;
// Meia transição da troca de página, como no protótipo (cerca de 140 ms).
const HALF_TRANSITION_MS = 70;
const SHIFT_PX = 12;
// As setas aparecem com o ponteiro sobre o desenho ou com o foco nele.
const NAV_FADE_MS = 150;

const NO_LED = Object.freeze({color: null, off: false, rainbow: false});

function iconFor(svg) {
    return Gio.BytesIcon.new(new GLib.Bytes(new TextEncoder().encode(svg)));
}

// O que importa para o corpo do aparelho: quem é e o que ele sabe fazer.
function bodyKey(device) {
    return device ? `${device.id}|${device.capabilities.join(',')}` : '';
}

export const HaloCarousel = GObject.registerClass({
    Signals: {
        'device-changed': {param_types: [GObject.TYPE_JSOBJECT]},
    },
}, class HaloCarousel extends St.BoxLayout {
    _init() {
        super._init({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-carousel',
            x_expand: true,
        });
        this._devices = [];
        this._index = null;
        this._led = NO_LED;
        this._svg = null;
        this._gate = null;
        this._emittedKey = null;

        // Bloco do aparelho: desenho, nome e status, indicador de página.
        this._tile = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-tile halo-device-tile',
            x_expand: true,
        });
        this.add_child(this._tile);

        this._stage = new St.Widget({
            style_class: 'halo-stage',
            layout_manager: new Clutter.BinLayout(),
            reactive: true,
            can_focus: true,
            track_hover: true,
            x_expand: true,
        });
        // O ícone fica no tamanho natural, centrado pelo constraint, e o
        // contêiner corta o que passa da altura do desenho.
        this._artClip = new St.Widget({
            style_class: 'halo-art-clip',
            layout_manager: new Clutter.FixedLayout(),
            clip_to_allocation: true,
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._art = new St.Icon({style_class: 'halo-art', icon_size: ART_SIZE});
        this._art.add_constraint(new Clutter.AlignConstraint({
            source: this._artClip,
            align_axis: Clutter.AlignAxis.BOTH,
            factor: 0.5,
        }));
        this._artClip.add_child(this._art);
        this._prev = this._navButton('go-previous-symbolic', 'Aparelho anterior',
            Clutter.ActorAlign.START, -1);
        this._next = this._navButton('go-next-symbolic', 'Próximo aparelho',
            Clutter.ActorAlign.END, 1);
        this._stage.add_child(this._artClip);
        this._stage.add_child(this._prev);
        this._stage.add_child(this._next);
        this._stage.connect('scroll-event', (_actor, event) => this._onScroll(event));
        this._stage.connect('key-press-event', (_actor, event) => this._onKey(event));
        this._stage.connect('notify::hover', () => this._syncNav());
        this._focusId = global.stage.connect('notify::key-focus', () => this._syncNav());
        this._tile.add_child(this._stage);

        const meta = new St.BoxLayout({style_class: 'halo-meta', x_expand: true});
        this._name = new St.Label({style_class: 'halo-device-name', x_expand: true,
            y_align: Clutter.ActorAlign.CENTER});
        const status = new St.BoxLayout({style_class: 'halo-chip',
            y_align: Clutter.ActorAlign.CENTER});
        status.add_child(new St.Widget({style_class: 'halo-chip-dot',
            y_align: Clutter.ActorAlign.CENTER}));
        this._status = new St.Label({style_class: 'halo-chip-text',
            y_align: Clutter.ActorAlign.CENTER});
        status.add_child(this._status);
        this._statusBox = status;
        meta.add_child(this._name);
        meta.add_child(status);
        this._tile.add_child(meta);

        this._pager = new St.BoxLayout({style_class: 'halo-pager',
            x_align: Clutter.ActorAlign.CENTER});
        this._tile.add_child(this._pager);

        // Público: o que fica abaixo do aparelho atual (avisos, depois os
        // controles do LED). Quem preenche é o indicador, no device-changed.
        this.body = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-device-body',
            x_expand: true,
        });
        this.add_child(this.body);

        this._empty = new St.Label({style_class: 'halo-tile halo-empty',
            text: 'Nenhum aparelho encontrado', x_expand: true});
        this.add_child(this._empty);

        this.connect('destroy', () => {
            global.stage.disconnect(this._focusId);
            this._focusId = 0;
        });

        this._render();
    }

    get device() {
        return this._index === null ? null : this._devices[this._index];
    }

    _navButton(iconName, label, align, dir) {
        const button = new St.Button({
            style_class: 'halo-nav',
            accessible_name: label,
            can_focus: true,
            opacity: 0,
            x_align: align,
            y_align: Clutter.ActorAlign.CENTER,
            x_expand: true,
            y_expand: true,
            child: new St.Icon({icon_name: iconName, style_class: 'halo-nav-icon'}),
        });
        button.connect('clicked', () => this._step(dir));
        return button;
    }

    // Como o .stage:hover .nav e o .stage:focus-within .nav do protótipo.
    _syncNav() {
        const focus = global.stage.key_focus;
        const shown = this._stage.hover || (focus !== null && this._stage.contains(focus));
        const opacity = shown ? 255 : 0;
        for (const nav of [this._prev, this._next]) {
            if (nav.opacity !== opacity)
                nav.ease({opacity, duration: NAV_FADE_MS, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        }
    }

    // A lista completa do daemon; mantém o aparelho atual pelo id.
    setDevices(devices) {
        const prevId = this.device?.id ?? null;
        this._devices = presentDevices(devices);
        this._index = selectAfterChange(prevId, devices);
        this._render();
        this._emitIfChanged();
    }

    // A pintura do anel ({color, off, rainbow}), de ledPaint(lastApplied).
    setLed(led) {
        this._led = led ?? NO_LED;
        this._renderArt();
    }

    _step(dir) {
        const n = this._devices.length;
        if (n < 2 || this._index === null)
            return;
        const index = dir > 0 ? nextIndex(this._index, n) : prevIndex(this._index, n);
        this.select(index, dir);
    }

    select(index, dir = 0) {
        if (this._index === null || index === this._index)
            return;
        this._index = index;
        this._renderMeta();
        this._renderPager();
        this._emitIfChanged();
        if (dir === 0) {
            this._renderArt();
            return;
        }
        // Sai para um lado e entra pelo outro; uma troca nova cancela a anterior,
        // e só a que termina redesenha (a cancelada pode ser a do destroy()).
        this._art.remove_all_transitions();
        this._art.ease({
            opacity: 0,
            translation_x: -dir * SHIFT_PX,
            duration: HALF_TRANSITION_MS,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: isFinished => {
                if (!isFinished)
                    return;
                this._renderArt();
                this._art.translation_x = dir * SHIFT_PX;
                this._art.ease({
                    opacity: 255,
                    translation_x: 0,
                    duration: HALF_TRANSITION_MS,
                    mode: Clutter.AnimationMode.EASE_OUT_QUAD,
                });
            },
        });
    }

    _onScroll(event) {
        let dy;
        switch (event.get_scroll_direction()) {
        case Clutter.ScrollDirection.UP:
            dy = -DISCRETE_DELTA;
            break;
        case Clutter.ScrollDirection.DOWN:
            dy = DISCRETE_DELTA;
            break;
        case Clutter.ScrollDirection.SMOOTH:
            dy = event.get_scroll_delta()[1] * SMOOTH_SCALE;
            break;
        default:
            return Clutter.EVENT_PROPAGATE;
        }
        if (this._devices.length < 2)
            return Clutter.EVENT_PROPAGATE;
        const {gate, step} = wheelStep(this._gate, dy, GLib.get_monotonic_time() / 1000);
        this._gate = gate;
        if (step !== 0)
            this._step(step);
        return Clutter.EVENT_STOP;
    }

    // Só Esquerda e Direita: Cima e Baixo andam entre os itens do menu.
    _onKey(event) {
        const key = event.get_key_symbol();
        if (key === Clutter.KEY_Left) {
            this._step(-1);
            return Clutter.EVENT_STOP;
        }
        if (key === Clutter.KEY_Right) {
            this._step(1);
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    _emitIfChanged() {
        const key = bodyKey(this.device);
        if (key === this._emittedKey)
            return;
        this._emittedKey = key;
        this.emit('device-changed', this.device);
    }

    _render() {
        const empty = this.device === null;
        this._empty.visible = empty;
        this._tile.visible = !empty;
        this.body.visible = !empty;
        this._renderMeta();
        this._renderPager();
        this._renderArt();
    }

    _renderMeta() {
        const device = this.device;
        if (!device)
            return;
        this._name.text = device.name;
        const status = statusText(device);
        this._status.text = status;
        if (status === 'Disponível')
            this._statusBox.add_style_class_name('halo-chip-ok');
        else
            this._statusBox.remove_style_class_name('halo-chip-ok');
        const multi = this._devices.length > 1;
        this._prev.visible = multi;
        this._next.visible = multi;
    }

    _renderPager() {
        const n = this._devices.length;
        this._pager.visible = n > 1;
        if (this._pager.get_n_children() !== n) {
            this._pager.destroy_all_children();
            for (let i = 0; i < n; i++) {
                const dot = new St.Button({style_class: 'halo-page-dot', can_focus: true});
                dot.connect('clicked', () => this.select(i, i > this._index ? 1 : -1));
                this._pager.add_child(dot);
            }
        }
        this._pager.get_children().forEach((dot, i) => {
            dot.accessible_name = `Mostrar ${this._devices[i].name}`;
            if (i === this._index)
                dot.add_style_pseudo_class('checked');
            else
                dot.remove_style_pseudo_class('checked');
        });
    }

    _renderArt() {
        const device = this.device;
        if (!device)
            return;
        const svg = deviceSvg(device, this._led);
        if (svg === this._svg)
            return;
        this._svg = svg;
        this._art.gicon = iconFor(svg);
    }
});
