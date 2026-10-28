// SPDX-License-Identifier: GPL-2.0-or-later
//
// Regras da janela de preferências, sem GTK: o prefs.js só as aplica.

import {CPU} from './temperature.js';

export const COOLANT_HINT = 'Padrão. Muda pouco: segue o líquido do cooler.';
export const CPU_HINT = 'Temperatura da CPU publicada pelo halod.';
export const CPU_MISSING_HINT = 'Indisponível: o halod ainda não fornece a temperatura da CPU.';

// A opção CPU: habilitada quando o halod publica a temperatura da CPU ou
// quando ela já é a fonte escolhida, para não prender o usuário numa opção
// que ele não consegue desmarcar nem confirmar.
export function cpuOption(published, source) {
    return {
        sensitive: published || source === CPU,
        hint: published ? CPU_HINT : CPU_MISSING_HINT,
    };
}
