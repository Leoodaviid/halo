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
import {ledPaint} from './lib/art.js';
import {lockedText} from './lib/carousel.js';
import {journalText} from './lib/journal.js';
import {barText} from './lib/temperature.js';
import {indicatorDimmed, panelView} from './lib/view.js';

// Opacidade do indicador com o daemon fora do ar.
const DIMMED_OPACITY = 128;
// Liga/desliga desabilitado, como o .switch:disabled do protótipo (.4).
const DISABLED_OPACITY = 102;

// Estado antes de qualquer resposta do D-Bus.
export const INITIAL_STATE = Object.freeze({
    owner: false,
    telemetry: null,
    devicesPresent: 0,
    devices: [],
    lastApplied: null,
    unit: 'unknown',
    journal: null,
});

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
    // handlers: { start(), journal(), back() }, chamados pelos botões do
    // painel. Sem eles, os botões não fazem nada.
    _init(extension, handlers = {}) {
        super._init(0.5, 'halo');
        this._handlers = handlers;

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
        // Cabeçalho: o título e o liga/desliga. Nesta versão o liga/desliga
        // fica desabilitado em todos os aparelhos.
        const header = new PopupMenu.PopupBaseMenuItem({
            reactive: false, can_focus: false, style_class: 'halo-item halo-header',
        });
        header.add_child(new St.Label({
            text: 'halo',
            style_class: 'halo-title',
            x_expand: true,
            y_align: Clutter.ActorAlign.CENTER,
        }));
        this._power = new PopupMenu.Switch(false);
        this._power.reactive = false;
        this._power.can_focus = false;
        this._power.add_style_pseudo_class('insensitive');
        this._power.opacity = DISABLED_OPACITY;
        this._power.y_align = Clutter.ActorAlign.CENTER;
        this._power.accessible_name = 'Liga e desliga o LED';
        header.add_child(this._power);
        this.menu.addMenuItem(header);

        // Com o daemon no ar: o carrossel e, abaixo, o corpo do aparelho.
        this._carouselItem = new PopupMenu.PopupBaseMenuItem({
            reactive: false, can_focus: false, style_class: 'halo-item',
        });
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
        if (!device)
            return;
        if (device.capabilities.includes('led')) {
            const note = new St.BoxLayout({
                orientation: Clutter.Orientation.VERTICAL,
                style_class: 'halo-tile halo-note',
                x_expand: true,
            });
            note.add_child(this._wrapped('Os controles do LED chegam na próxima versão do painel.',
                'halo-note-text'));
            body.add_child(note);
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

    update(state) {
        if (this._destroyed)
            return;
        const text = state.owner ? barText(state.telemetry) : '';
        this._temp.text = text;
        this._temp.visible = text !== '';
        this._box.opacity = indicatorDimmed(state) ? DIMMED_OPACITY : 255;

        const view = panelView(state);
        const showJournal = view.kind === 'down' && state.journal !== null;
        this._main.actor.visible = !showJournal;
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
            this._carousel.setLed(ledPaint(state.lastApplied));
        }
        this._reason.visible = down && view.reason !== null;
        this._reason.label.text = down && view.reason ? view.reason : '';
        this._actions.visible = down;
        this._start.reactive = down && view.canStart;
        this._openJournal.reactive = down && view.canJournal;
    }
});
