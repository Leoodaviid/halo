// SPDX-License-Identifier: GPL-2.0-or-later
//
// Indicador na barra e painel mínimo. Só desenha: o que mostrar vem das
// funções puras de lib/, a partir do estado recebido em update().

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {journalText} from './lib/journal.js';
import {barText} from './lib/temperature.js';
import {indicatorDimmed, panelView} from './lib/view.js';

// Opacidade do indicador com o daemon fora do ar.
const DIMMED_OPACITY = 128;

// Estado antes de qualquer resposta do D-Bus.
export const INITIAL_STATE = Object.freeze({
    owner: false,
    telemetry: null,
    devicesPresent: 0,
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
        this.update(INITIAL_STATE);
    }

    _buildMenu() {
        const title = new PopupMenu.PopupMenuItem('halo', {reactive: false});
        title.label.add_style_class_name('halo-title');
        this.menu.addMenuItem(title);

        // Estado normal: a mensagem, o motivo e as ações.
        this._main = new PopupMenu.PopupMenuSection();
        this._message = new PopupMenu.PopupMenuItem('', {reactive: false});
        this._reason = new PopupMenu.PopupMenuItem('', {reactive: false});
        this._reason.label.add_style_class_name('halo-reason');
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

    update(state) {
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
        this._reason.visible = down && view.reason !== null;
        this._reason.label.text = down && view.reason ? view.reason : '';
        this._actions.visible = down;
        this._start.reactive = down && view.canStart;
        this._openJournal.reactive = down && view.canJournal;
    }
});
