// SPDX-License-Identifier: GPL-2.0-or-later
//
// Extensão do halo: um indicador na barra superior que só conversa com o
// halod pelo D-Bus da sessão. Tudo o que enable() cria, disable() destrói.

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {HaloDaemon} from './daemon.js';
import {HaloIndicator, INITIAL_STATE} from './indicator.js';

export default class HaloExtension extends Extension {
    enable() {
        this._indicator = new HaloIndicator(this, {
            start: () => this._daemon?.startService(),
            journal: () => this._daemon?.showJournal(),
            back: () => this._daemon?.hideJournal(),
        });
        this._daemon = new HaloDaemon(INITIAL_STATE, state => this._indicator?.update(state));
        // Com o daemon fora do ar, o estado da unidade é conferido de novo a
        // cada abertura do painel (ela pode ter sido instalada nesse meio-tempo).
        this._menuOpen = this._indicator.menu.connect('open-state-changed', (_menu, open) => {
            if (open && !this._daemon.state.owner)
                this._daemon.refreshUnit();
        });
        Main.panel.addToStatusArea(this.uuid, this._indicator);
        this._daemon.start();
    }

    disable() {
        this._daemon?.destroy();
        this._daemon = null;
        if (this._menuOpen)
            this._indicator?.menu.disconnect(this._menuOpen);
        this._menuOpen = null;
        this._indicator?.destroy();
        this._indicator = null;
    }
}
