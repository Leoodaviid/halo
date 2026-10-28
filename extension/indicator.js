// SPDX-License-Identifier: GPL-2.0-or-later
//
// Indicador na barra e painel mínimo. Só desenha: o que mostrar vem das
// funções puras de lib/, a partir do estado recebido em update().

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import Pango from 'gi://Pango';
import St from 'gi://St';

import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {HaloCarousel} from './carousel.js';
import {HaloLedControls} from './ledControls.js';
import {ledPaint} from './lib/art.js';
import {EMPTY as LED_EMPTY, controlsView} from './lib/ledState.js';
import {lockedText} from './lib/carousel.js';
import {journalText} from './lib/journal.js';
import {COOLANT, barText} from './lib/temperature.js';
import {indicatorDimmed, panelView} from './lib/view.js';

// Opacidade do indicador com o daemon fora do ar.
const DIMMED_OPACITY = 128;
// Liga/desliga insensível, como o .switch:disabled do protótipo (.4).
const DISABLED_OPACITY = 102;

// Medidas do corpo do menu e dos nossos itens, inline: eles também têm as
// classes do tema do Shell (.popup-menu-content, .popup-menu-item), e um tema
// de usuário (User Themes) vence o stylesheet.css da extensão. Painel de
// 320 px, como o .material: 304 px de conteúdo e 8 px de padding, que
// substituem o padding do tema.
const PANEL_STYLE = 'width: 304px; padding: 8px; spacing: 8px; font-size: 13px;';
const ITEM_STYLE = 'padding: 0; spacing: 0;';
const HEADER_STYLE = 'padding: 4px 4px 2px 6px; spacing: 0;';

// Estado antes de qualquer resposta do D-Bus.
export const INITIAL_STATE = Object.freeze({
    owner: false,
    telemetry: null,
    devicesPresent: 0,
    devices: [],
    lastApplied: null,
    led: LED_EMPTY,
    unit: 'unknown',
    journal: null,
});

function hasLed(device) {
    return device?.id === 'waterforce' && device.capabilities.includes('led');
}

function button(label, onClick) {
    const b = new St.Button({
        label,
        style_class: 'button halo-button',
        can_focus: true,
        x_expand: true,
    });
    b.connect('clicked', () => onClick());
    return b;
}

export const HaloIndicator = GObject.registerClass(
class HaloIndicator extends PanelMenu.Button {
    // handlers: { start(), journal(), back(), ledDraft(s), ledCommit(s),
    // ledPower(), prefs() }, chamados pelos botões e controles do painel. Sem eles,
    // nada acontece. source: a fonte da temperatura na barra (COOLANT ou CPU).
    _init(extension, handlers = {}, source = COOLANT) {
        super._init(0.5, 'halo');
        this._handlers = handlers;
        this._source = source;
        this._state = INITIAL_STATE;
        this._ledControls = null;

        const box = new St.BoxLayout({style_class: 'panel-status-indicators-box halo-indicator'});
        const gicon = Gio.FileIcon.new(
            extension.dir.get_child('icons').get_child('halo-symbolic.svg'));
        box.add_child(new St.Icon({gicon, style_class: 'system-status-icon'}));
        this._temp = new St.Label({
            style_class: 'halo-temp',
            y_align: Clutter.ActorAlign.CENTER,
        });
        box.add_child(this._temp);
        this.add_child(box);
        this._box = box;

        this.menu.box.add_style_class_name('halo-panel');
        this.menu.box.style = PANEL_STYLE;
        this._buildMenu();
        // O Shell destrói o indicador ao sair sem chamar disable(), e o daemon
        // ainda pode mandar um estado: depois do destroy, update() não desenha.
        this._destroyed = false;
        this.connect('destroy', () => {
            this._destroyed = true;
        });
        this.update(INITIAL_STATE);
    }

    _buildMenu() {
        // Cabeçalho: o título e o liga/desliga do LED do aparelho atual,
        // insensível nos aparelhos sem LED, sem o halod, com um pedido em voo
        // e durante a leitura do perfil.
        const header = new PopupMenu.PopupBaseMenuItem({
            reactive: false, can_focus: false, style_class: 'halo-item halo-header',
        });
        header.style = HEADER_STYLE;
        header.add_child(new St.Label({
            text: 'halo',
            style_class: 'halo-title',
            x_expand: true,
            y_align: Clutter.ActorAlign.CENTER,
        }));
        this._power = new PopupMenu.Switch(false);
        this._power.reactive = false;
        this._powerButton = new St.Button({
            style_class: 'halo-power',
            child: this._power,
            can_focus: true,
            y_align: Clutter.ActorAlign.CENTER,
            accessible_name: 'Liga e desliga o LED',
        });
        this._powerButton.connect('clicked', () => this._handlers.ledPower?.());
        header.add_child(this._powerButton);
        this.menu.addMenuItem(header);

        // Com o daemon no ar: o carrossel e, abaixo, o corpo do aparelho.
        this._carouselItem = new PopupMenu.PopupBaseMenuItem({
            reactive: false, can_focus: false, style_class: 'halo-item',
        });
        this._carouselItem.style = ITEM_STYLE;
        this._carousel = new HaloCarousel();
        this._carousel.connect('device-changed', (_c, device) => this._fillBody(device));
        this._carouselItem.add_child(this._carousel);
        this.menu.addMenuItem(this._carouselItem);

        // Estado normal: a mensagem, o motivo e as ações.
        this._main = new PopupMenu.PopupMenuSection();
        this._message = new PopupMenu.PopupMenuItem('', {reactive: false});
        this._reason = new PopupMenu.PopupMenuItem('', {reactive: false});
        this._reason.label.add_style_class_name('halo-reason');
        for (const item of [this._message, this._reason])
            item.label.clutter_text.line_wrap = true;
        this._actions = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        const actions = new St.BoxLayout({style_class: 'halo-actions', x_expand: true});
        this._start = button('Iniciar halod', () => this._handlers.start?.());
        this._openJournal = button('Ver registro', () => this._handlers.journal?.());
        actions.add_child(this._start);
        actions.add_child(this._openJournal);
        this._actions.add_child(actions);
        this._main.addMenuItem(this._message);
        this._main.addMenuItem(this._reason);
        this._main.addMenuItem(this._actions);
        this.menu.addMenuItem(this._main);

        // Rodapé da página principal, com o daemon no ar ou fora do ar: a
        // linha e o item das preferências (o .foot do protótipo).
        this._foot = new PopupMenu.PopupBaseMenuItem({
            reactive: false, can_focus: false, style_class: 'halo-item',
        });
        this._foot.style = ITEM_STYLE;
        const foot = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-foot',
            x_expand: true,
        });
        foot.add_child(new St.Widget({style_class: 'halo-hr', x_expand: true}));
        const prefs = new St.Button({
            style_class: 'halo-menu-row',
            can_focus: true,
            x_expand: true,
            child: new St.Label({
                text: 'Preferências do halo…',
                x_expand: true,
                y_align: Clutter.ActorAlign.CENTER,
            }),
        });
        prefs.connect('clicked', () => {
            this.menu.close();
            this._handlers.prefs?.();
        });
        foot.add_child(prefs);
        this._foot.add_child(foot);
        this.menu.addMenuItem(this._foot);

        // Registro do halod, só leitura, com "Voltar".
        this._journalSection = new PopupMenu.PopupMenuSection();
        const journalItem = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        this._journalLabel = new St.Label({style_class: 'halo-journal', x_expand: true});
        this._journalLabel.clutter_text.line_wrap = true;
        journalItem.add_child(this._journalLabel);
        const backItem = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        backItem.add_child(button('Voltar', () => this._handlers.back?.()));
        this._journalSection.addMenuItem(journalItem);
        this._journalSection.addMenuItem(backItem);
        this.menu.addMenuItem(this._journalSection);
    }

    // O que fica abaixo do aparelho atual. Sem a capacidade `led`, o aviso de
    // controle em desenvolvimento (o .locked do protótipo); com ela, por
    // enquanto, um aviso provisório até os controles do LED.
    _fillBody(device) {
        const body = this._carousel.body;
        body.destroy_all_children();
        this._ledControls = null;
        this._renderPower();
        if (!device)
            return;
        // O único LED no D-Bus do halod é o do Waterforce.
        if (hasLed(device)) {
            this._ledControls = new HaloLedControls({
                draft: s => this._handlers.ledDraft?.(s),
                commit: s => this._handlers.ledCommit?.(s),
            });
            this._ledControls.update(this._state.led, this._state.owner);
            body.add_child(this._ledControls);
            return;
        }
        const locked = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'halo-tile halo-locked',
            x_expand: true,
        });
        locked.add_child(new St.Icon({
            icon_name: 'changes-prevent-symbolic',
            style_class: 'halo-locked-icon',
            x_align: Clutter.ActorAlign.CENTER,
        }));
        locked.add_child(this._wrapped('Controle em desenvolvimento', 'halo-locked-title', true));
        locked.add_child(this._wrapped(lockedText(device), 'halo-locked-text', true));
        body.add_child(locked);
    }

    _wrapped(text, styleClass, center = false) {
        const label = new St.Label({text, style_class: styleClass, x_expand: true});
        label.clutter_text.line_wrap = true;
        if (center)
            label.clutter_text.line_alignment = Pango.Alignment.CENTER;
        return label;
    }

    // O liga/desliga segue o LED do aparelho atual.
    _renderPower() {
        const device = this._carousel.device;
        const view = controlsView(this._state.led, this._state.owner);
        const sensitive = hasLed(device) && view.power.sensitive;
        this._power.state = hasLed(device) && view.power.on;
        this._powerButton.reactive = sensitive;
        this._powerButton.can_focus = sensitive;
        this._powerButton.opacity = sensitive ? 255 : DISABLED_OPACITY;
        if (sensitive)
            this._power.remove_style_pseudo_class('insensitive');
        else
            this._power.add_style_pseudo_class('insensitive');
    }

    // Troca a fonte da barra e redesenha com o último estado.
    setSource(source) {
        if (this._destroyed)
            return;
        this._source = source;
        this._renderTemp();
    }

    _renderTemp() {
        const state = this._state;
        const text = state.owner ? barText(state.telemetry, this._source) : '';
        this._temp.text = text;
        this._temp.visible = text !== '';
    }

    update(state) {
        if (this._destroyed)
            return;
        this._state = state;
        this._renderTemp();
        this._box.opacity = indicatorDimmed(state) ? DIMMED_OPACITY : 255;

        const view = panelView(state);
        const showJournal = view.kind === 'down' && state.journal !== null;
        this._main.actor.visible = !showJournal;
        this._foot.visible = !showJournal;
        this._journalSection.actor.visible = showJournal;
        if (showJournal)
            this._journalLabel.text = journalText(state.journal);

        this._message.label.text = view.message;
        const down = view.kind === 'down';
        // O carrossel substitui a contagem com o daemon no ar.
        this._carouselItem.visible = !down;
        this._message.visible = down;
        if (!down) {
            this._carousel.setDevices(state.devices);
            // O anel segue o rascunho: durante o gesto ele já mostra a cor.
            this._carousel.setLed(ledPaint(state.led.draft));
            this._ledControls?.update(state.led, state.owner);
        }
        this._renderPower();
        this._reason.visible = down && view.reason !== null;
        this._reason.label.text = down && view.reason ? view.reason : '';
        this._actions.visible = down;
        this._start.reactive = down && view.canStart;
        this._openJournal.reactive = down && view.canJournal;
    }
});
