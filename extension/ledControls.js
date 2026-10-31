// SPDX-License-Identifier: GPL-2.0-or-later
//
// Controles do LED do Waterforce, no corpo do aparelho: efeito, cor (cores
// prontas, matiz e hex), brilho e velocidade. Só desenha e liga sinais: o que
// mostrar vem de controlsView (lib/ledState.js), e cada fim de gesto chama
// commit(setting), que passa pela fila do daemon. Durante o gesto, só
// draft(setting), que muda a interface e o desenho, sem pedido nenhum. Acima
// dos controles, a faixa de erro (lib/feedback.js) com "Tentar de novo", que
// chama retry(). Medidas e estrutura do protótipo (docs/ui/panel-prototype.html:
// o .banner, os dois .tile dos controles, .swatches, .hue, .cc).

import Cairo from 'cairo';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Pango from 'gi://Pango';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as Slider from 'resource:///org/gnome/shell/ui/slider.js';

import {errorView} from './lib/feedback.js';
import {
    EFFECTS, SWATCHES, controlsView, effectChoice, hueOf, hueToHex, parseHex, withBrightness,
    withColor, withSpeed,
} from './lib/ledState.js';

// Teclas e cliques seguidos viram um gesto só depois desse intervalo, como o
// commitColorSoon do protótipo.
const GROUP_MS = 300;
// Opacidade dos controles travados, como o .dim do protótipo.
const DIM_OPACITY = 102;
const HUE_STEP = 10;
// Medidas que o tema do Shell também define (.slider, StEntry): inline, para
// valerem em qualquer tema; no Yaru da sessão, as regras do tema venciam as
// do stylesheet.css. As cores continuam no CSS de cada tema.
const SLIDER_STYLE = 'height: 28px; -slider-handle-radius: 14px;';
const HEX_STYLE = 'width: 96px; height: 24px; min-height: 0; padding: 0 8px; border-radius: 6px;';

function roundedRect(cr, x, y, w, h, r) {
    cr.newSubPath();
    cr.arc(x + w - r, y + r, r, -Math.PI / 2, 0);
    cr.arc(x + w - r, y + h - r, r, 0, Math.PI / 2);
    cr.arc(x + r, y + h - r, r, Math.PI / 2, Math.PI);
    cr.arc(x + r, y + r, r, Math.PI, 1.5 * Math.PI);
    cr.closePath();
}

function label(text, styleClass, params = {}) {
    return new St.Label({text, style_class: styleClass, y_align: Clutter.ActorAlign.CENTER, ...params});
}

// Um temporizador de uma vez, que o destroy() remove.
class Later {
    constructor() {
        this._id = 0;
        this._fn = null;
    }

    schedule(fn) {
        this.cancel();
        this._fn = fn;
        this._id = GLib.timeout_add(GLib.PRIORITY_DEFAULT, GROUP_MS, () => {
            this._id = 0;
            const f = this._fn;
            this._fn = null;
            f();
            return GLib.SOURCE_REMOVE;
        });
    }

    // Roda já o que estava esperando (o gesto não se perde ao trocar de aparelho).
    flush() {
        const f = this._fn;
        this.cancel();
        f?.();
    }

    cancel() {
        if (this._id)
            GLib.source_remove(this._id);
        this._id = 0;
        this._fn = null;
    }
}

// Slider inteiro de min a max, no estilo da Central de Controle (.cc): uma
// pílula com o preenchimento até o valor. Aplica no drag-end; a roda do mouse
// fica desligada, porque no Shell ela muda o valor sem drag-end.
const HaloSlider = GObject.registerClass({
    Signals: {
        'int-changed': {param_types: [GObject.TYPE_INT]},
        'key-step': {},
    },
}, class HaloSlider extends Slider.Slider {
    _init(min, max, accessibleName) {
        super._init(0);
        this.add_style_class_name('halo-led-slider');
        this.style = SLIDER_STYLE;
        this.accessible_name = accessibleName;
        this._min = min;
        this._max = max;
        this._int = min;
        this._quiet = false;
        this.connect('notify::value', () => {
            const v = Math.round(this._min + this.value * (this._max - this._min));
            if (v === this._int)
                return;
            this._int = v;
            if (!this._quiet)
                this.emit('int-changed', v);
        });
    }

    setInt(v) {
        this._int = v;
        this._quiet = true;
        this.value = (v - this._min) / (this._max - this._min);
        this._quiet = false;
    }

    vfunc_scroll_event() {
        return Clutter.EVENT_PROPAGATE;
    }

    // Esquerda e Direita andam um inteiro; Cima e Baixo ficam com o menu.
    vfunc_key_press_event(event) {
        const key = event.get_key_symbol();
        if (key !== Clutter.KEY_Left && key !== Clutter.KEY_Right)
            return Clutter.EVENT_PROPAGATE;
        const rtl = this.get_text_direction() === Clutter.TextDirection.RTL;
        const up = (key === Clutter.KEY_Right) !== rtl;
        const v = Math.min(this._max, Math.max(this._min, this._int + (up ? 1 : -1)));
        if (v !== this._int) {
            this.setInt(v);
            this.emit('int-changed', v);
            this.emit('key-step');
        }
        return Clutter.EVENT_STOP;
    }

    vfunc_repaint() {
        const cr = this.get_context();
        const node = this.get_theme_node();
        const [w, h] = this.get_surface_size();
        const r = h / 2;
        cr.setSourceColor(node.get_color('-halo-slider-track'));
        roundedRect(cr, 0, 0, w, h, r);
        cr.fill();
        const p = (this._int - this._min) / (this._max - this._min);
        const fill = h + (w - h) * p;
        cr.setSourceColor(node.get_color('-halo-slider-fill'));
        roundedRect(cr, 0, 0, fill, h, r);
        cr.fill();
        cr.$dispose();
    }
});

// Barra de matiz (.hue): o gradiente e o marcador, num St.DrawingArea.
const HueBar = GObject.registerClass({
    Signals: {
        'hue-changed': {param_types: [GObject.TYPE_DOUBLE]},
        'hue-end': {},
        'key-step': {},
    },
}, class HueBar extends St.DrawingArea {
    _init() {
        super._init({
            style_class: 'halo-led-hue',
            reactive: true,
            can_focus: true,
            x_expand: true,
            accessible_name: 'Matiz',
        });
        this._hue = 0;
        this._pan = new Clutter.PanGesture();
        this._pan.set_begin_threshold(0);
        this._pan.connect('recognize', () => {
            this._grab = global.stage.grab(this);
            this._fromPointer();
        });
        this._pan.connect('pan-update', () => this._fromPointer());
        this._pan.connect('end', () => {
            this._grab?.dismiss();
            this._grab = null;
            this.emit('hue-end');
        });
        this.add_action(this._pan);
    }

    setHue(h) {
        if (h === this._hue)
            return;
        this._hue = h;
        this.queue_repaint();
    }

    _fromPointer() {
        const {x} = this._pan.get_centroid();
        const [w] = this.get_size();
        const h = Math.max(0, Math.min(359.9, (x - 10) / (w - 20) * 360));
        this.setHue(h);
        this.emit('hue-changed', h);
    }

    vfunc_key_press_event(event) {
        const key = event.get_key_symbol();
        if (key !== Clutter.KEY_Left && key !== Clutter.KEY_Right)
            return Clutter.EVENT_PROPAGATE;
        const h = (this._hue + (key === Clutter.KEY_Right ? HUE_STEP : -HUE_STEP) + 360) % 360;
        this.setHue(h);
        this.emit('hue-changed', h);
        this.emit('key-step');
        return Clutter.EVENT_STOP;
    }

    vfunc_repaint() {
        const cr = this.get_context();
        const [w, h] = this.get_surface_size();
        const grad = new Cairo.LinearGradient(0, 0, w, 0);
        ['#ff0000', '#ffff00', '#00ff00', '#00ffff', '#0000ff', '#ff00ff', '#ff0000'].forEach((c, i) => {
            const n = parseInt(c.slice(1), 16);
            grad.addColorStopRGB(i / 6, (n >> 16) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255);
        });
        const barH = h * 0.4;
        roundedRect(cr, 0, (h - barH) / 2, w, barH, barH / 2);
        cr.setSource(grad);
        cr.fill();
        const r = h / 2;
        const x = r + (w - 2 * r) * (this._hue / 360);
        cr.setSourceRGBA(0, 0, 0, 0.25);
        cr.arc(x, r + 1, r - 0.5, 0, 2 * Math.PI);
        cr.fill();
        cr.setSourceRGB(1, 1, 1);
        cr.arc(x, r, r - 1, 0, 2 * Math.PI);
        cr.fill();
        cr.$dispose();
    }
});

export const HaloLedControls = GObject.registerClass(
class HaloLedControls extends St.BoxLayout {
    // handlers: { draft(setting), commit(setting), retry() }.
    _init(handlers) {
        super._init({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-led-controls',
            x_expand: true,
        });
        this._handlers = handlers;
        this._led = null;
        this._view = null;
        this._later = new Later();
        this._hexCancel = false;
        // O usuário editou o hex desde o último envio ou desenho: só assim a
        // perda do foco envia (senão um valor que já falhou sairia de novo).
        this._hexDirty = false;
        this._hexSetting = false;

        this._banner = this._buildBanner();
        this.add_child(this._banner);

        this._unknown = this._note('Estado do LED desconhecido',
            'O halod não informou o que está aplicado. Escolha um efeito para começar.');
        this.add_child(this._unknown);

        // Bloco do efeito e da cor.
        const top = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-tile halo-led-tile',
            x_expand: true,
        });
        this.add_child(top);

        const effectRow = new St.BoxLayout({style_class: 'halo-led-row', x_expand: true});
        effectRow.add_child(label('Efeito', 'halo-led-label', {x_expand: true}));
        this._effectLabel = label('', 'halo-led-popup-text', {x_expand: true});
        const effectBox = new St.BoxLayout({x_expand: true});
        effectBox.add_child(this._effectLabel);
        effectBox.add_child(new St.Icon({icon_name: 'pan-down-symbolic', style_class: 'halo-led-popup-icon'}));
        this._effectButton = new St.Button({
            style_class: 'halo-led-popup',
            can_focus: true,
            child: effectBox,
            accessible_name: 'Efeito',
        });
        this._effectButton.connect('clicked', () => this._effectMenu.toggle());
        effectRow.add_child(this._effectButton);
        top.add_child(effectRow);
        this._buildEffectMenu();

        // Cor: só nos efeitos com cor (e esmaecida com o LED desligado).
        this._colorBox = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-led-split',
            x_expand: true,
        });
        const colorRow = new St.BoxLayout({style_class: 'halo-led-row', x_expand: true});
        colorRow.add_child(label('Cor', 'halo-led-label', {x_expand: true}));
        colorRow.add_child(label('Hex', 'halo-led-small'));
        this._hex = new St.Entry({
            style_class: 'halo-led-hex',
            style: HEX_STYLE,
            can_focus: true,
            accessible_name: 'Cor em hexadecimal',
        });
        this._hex.clutter_text.max_length = 7;
        this._hex.clutter_text.connect('activate', () => this._commitHex());
        this._hex.clutter_text.connect('key-focus-out', () => {
            if (this._hexCancel) {
                this._hexCancel = false;
                this._render();
                return;
            }
            if (this._hexDirty && !this._hex.has_style_class_name('halo-led-hex-bad'))
                this._commitHex();
        });
        this._hex.clutter_text.connect('text-changed', () => {
            if (this._hexSetting)
                return;
            this._hexDirty = true;
            this._hex.remove_style_class_name('halo-led-hex-bad');
        });
        this._hex.clutter_text.connect('key-press-event', (_t, event) => {
            if (event.get_key_symbol() !== Clutter.KEY_Escape)
                return Clutter.EVENT_PROPAGATE;
            this._hexCancel = true;
            this._hexDirty = false;
            this._hex.remove_style_class_name('halo-led-hex-bad');
            this._effectButton.grab_key_focus();
            return Clutter.EVENT_STOP;
        });
        colorRow.add_child(this._hex);
        this._colorBox.add_child(colorRow);

        this._swatches = new St.BoxLayout({style_class: 'halo-led-swatches', x_expand: true});
        for (const [hex, name] of SWATCHES) {
            const sw = new St.Button({
                style_class: 'halo-led-swatch',
                can_focus: true,
                accessible_name: name,
                child: new St.Widget({style_class: 'halo-led-swatch-color', style: `background-color: #${hex};`}),
            });
            sw._haloHex = hex;
            sw.connect('clicked', () => {
                this._setColor(hex);
                this._later.schedule(() => this._commit());
            });
            this._swatches.add_child(sw);
        }
        this._colorBox.add_child(this._swatches);

        this._hue = new HueBar();
        this._hue.connect('hue-changed', (_b, h) => this._setColor(hueToHex(h)));
        this._hue.connect('hue-end', () => this._commit());
        this._hue.connect('key-step', () => this._later.schedule(() => this._commit()));
        this._colorBox.add_child(this._hue);
        top.add_child(this._colorBox);

        // Paleta: o nome e as cores, só para leitura.
        this._paletteBox = new St.BoxLayout({style_class: 'halo-led-split', x_expand: true});
        this._paletteBox.add_child(label('Paleta', 'halo-led-label', {x_expand: true}));
        this._paletteDots = new St.BoxLayout({style_class: 'halo-led-palette', y_align: Clutter.ActorAlign.CENTER});
        this._paletteBox.add_child(this._paletteDots);
        this._paletteBox.add_child(label('definida pelo perfil', 'halo-led-small'));
        top.add_child(this._paletteBox);

        // Bloco do brilho e da velocidade.
        const bottom = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-tile halo-led-tile halo-led-sliders',
            x_expand: true,
        });
        this.add_child(bottom);
        [this._brightnessBox, this._brightness, this._brightnessValue] =
            this._sliderRow(bottom, 'Brilho', 'display-brightness-symbolic', 1, 10,
                v => withBrightness(this._led, v));
        [this._speedBox, this._speed, this._speedValue] =
            this._sliderRow(bottom, 'Velocidade', 'power-profile-performance-symbolic', 1, 5,
                v => withSpeed(this._led, v));
        this._top = top;
        this._bottom = bottom;

        this.connect('destroy', () => {
            this._later.flush();
            this._effectMenu.destroy();
        });
    }

    // A faixa de erro (o .banner): o ícone, o título, o texto e o botão.
    _buildBanner() {
        const banner = new St.BoxLayout({style_class: 'halo-banner', x_expand: true});
        banner.add_child(new St.Icon({
            icon_name: 'dialog-warning-symbolic',
            style_class: 'halo-banner-icon',
            y_align: Clutter.ActorAlign.START,
        }));
        const column = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-banner-body',
            x_expand: true,
        });
        // Numa caixa horizontal, o rótulo só pede a altura das linhas
        // quebradas sem as reticências.
        this._bannerTitle = label('', 'halo-banner-title', {x_expand: true});
        this._bannerText = label('', 'halo-banner-text', {x_expand: true});
        for (const l of [this._bannerTitle, this._bannerText]) {
            l.clutter_text.line_wrap = true;
            l.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
        }
        const retry = new St.Button({
            label: 'Tentar de novo',
            style_class: 'halo-text-btn',
            can_focus: true,
            x_align: Clutter.ActorAlign.START,
        });
        retry.connect('clicked', () => this._handlers.retry?.());
        column.add_child(this._bannerTitle);
        column.add_child(this._bannerText);
        column.add_child(retry);
        banner.add_child(column);
        return banner;
    }

    _note(title, text) {
        const box = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-tile halo-led-note',
            x_expand: true,
        });
        box.add_child(label(title, 'halo-led-note-title'));
        const body = label(text, 'halo-led-note-text', {x_expand: true});
        body.clutter_text.line_wrap = true;
        box.add_child(body);
        return box;
    }

    _sliderRow(parent, title, iconName, min, max, change) {
        const box = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-led-slider-box',
            x_expand: true,
        });
        const row = new St.BoxLayout({style_class: 'halo-led-row', x_expand: true});
        row.add_child(label(title, 'halo-led-label', {x_expand: true}));
        const value = label('', 'halo-led-small');
        row.add_child(value);
        box.add_child(row);

        const stack = new St.Widget({layout_manager: new Clutter.BinLayout(), x_expand: true});
        const slider = new HaloSlider(min, max, title);
        slider.connect('int-changed', (_s, v) => this._handlers.draft(change(v)));
        slider.connect('drag-end', () => this._commit());
        slider.connect('key-step', () => this._later.schedule(() => this._commit()));
        stack.add_child(slider);
        stack.add_child(new St.Icon({
            icon_name: iconName,
            style_class: 'halo-led-slider-icon',
            x_align: Clutter.ActorAlign.START,
            x_expand: true,
            y_align: Clutter.ActorAlign.CENTER,
        }));
        box.add_child(stack);
        parent.add_child(box);
        return [box, slider, value];
    }

    _buildEffectMenu() {
        this._effectMenu = new PopupMenu.PopupMenu(this._effectButton, 0.5, St.Side.TOP);
        this._effectMenu.actor.add_style_class_name('halo-led-effect-menu');
        this._effectItems = new Map();
        for (const [effect, name] of EFFECTS) {
            const item = new PopupMenu.PopupMenuItem(name);
            item.connect('activate', () => this._chooseEffect(effect));
            this._effectMenu.addMenuItem(item);
            this._effectItems.set(effect, item);
        }
        this._effectMenuManager = new PopupMenu.PopupMenuManager(this._effectButton);
        this._effectMenuManager.addMenu(this._effectMenu);
        Main.uiGroup.add_child(this._effectMenu.actor);
        this._effectMenu.actor.hide();
    }

    _chooseEffect(effect) {
        if (this._led?.draft?.effect === effect)
            return;
        this._handlers.commit(effectChoice(this._led, effect));
    }

    _setColor(hex) {
        const s = withColor(this._led, hex);
        if (s !== this._led.draft)
            this._handlers.draft(s);
    }

    _commitHex() {
        const hex = parseHex(this._hex.text);
        if (hex === null) {
            this._hex.add_style_class_name('halo-led-hex-bad');
            return;
        }
        this._hexDirty = false;
        this._setColor(hex);
        this._commit();
    }

    // Fim de gesto: o rascunho atual vai para a fila.
    _commit() {
        this._later.cancel();
        const draft = this._led?.draft;
        if (draft)
            this._handlers.commit(draft);
    }

    // O estado do LED (lib/ledState.js) e se o halod está no ar.
    update(led, owner) {
        this._led = led;
        this._view = controlsView(led, owner);
        this._render();
    }

    _render() {
        const v = this._view;
        if (!v)
            return;
        this._unknown.visible = v.unknown;
        this._effectLabel.text = v.effectLabel;
        for (const [effect, item] of this._effectItems) {
            item.setOrnament(effect === v.effect
                ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
        }

        this._colorBox.visible = v.showColor;
        this._setLocked(this._colorBox, v.locked);
        if (v.showColor) {
            for (const sw of this._swatches.get_children()) {
                if (sw._haloHex === v.color)
                    sw.add_style_pseudo_class('checked');
                else
                    sw.remove_style_pseudo_class('checked');
            }
            // Não apaga o que o usuário está digitando.
            if (!this._hexDirty) {
                this._hexSetting = true;
                this._hex.text = `#${v.color.toUpperCase()}`;
                this._hexSetting = false;
                this._hex.remove_style_class_name('halo-led-hex-bad');
            }
            this._hue.setHue(hueOf(v.color));
        }

        this._paletteBox.visible = v.palette !== null;
        if (v.palette) {
            this._paletteDots.destroy_all_children();
            for (const c of v.palette.colors) {
                this._paletteDots.add_child(new St.Widget({
                    style_class: 'halo-led-palette-color', style: `background-color: #${c};`,
                }));
            }
        }

        this._brightness.setInt(v.brightness);
        this._brightnessValue.text = v.brightnessText;
        this._setLocked(this._brightnessBox, v.locked);
        this._speed.setInt(v.speed);
        this._speedValue.text = v.speedText;
        // Velocidade que não se aplica: só a barra esmaece, como o
        // .cc[aria-disabled] do protótipo; com o LED desligado, a linha toda.
        this._setLocked(this._speedBox, v.locked);
        if (!v.locked && !v.speedEnabled) {
            this._speed.opacity = DIM_OPACITY;
            this._speed.reactive = false;
            this._speed.can_focus = false;
        } else {
            this._speed.opacity = 255;
        }

        this._banner.visible = v.error !== null;
        if (v.error !== null) {
            const e = errorView(v.error, this._led.failed?.kind === 'activate');
            this._bannerTitle.text = e.title;
            this._bannerText.text = e.text;
        }
    }

    // Travado: esmaecido e sem responder a clique, arraste ou teclado.
    _setLocked(box, locked) {
        box.opacity = locked ? DIM_OPACITY : 255;
        const interactive = [this._hex, this._hue, this._brightness, this._speed,
            ...this._swatches.get_children()];
        for (const actor of interactive) {
            if (box.contains(actor)) {
                actor.reactive = !locked;
                actor.can_focus = !locked;
            }
        }
    }
});
