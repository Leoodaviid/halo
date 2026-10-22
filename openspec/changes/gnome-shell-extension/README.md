# gnome-shell-extension

Fase 2.3: extensão GNOME Shell (GJS) com ícone na barra e painel, falando só com o D-Bus do daemon. Depende de halo-daemon e do protótipo aprovado em gnome-panel-prototype.

Segue o protótipo [`docs/ui/panel-prototype.html`](../../../docs/ui/panel-prototype.html) e a spec `gnome-panel`. A tabela de equivalência com o St está em [`docs/architecture.md`](../../../docs/architecture.md#painel-do-gnome-shell). Antes dela, o `halod` precisa publicar o `Devices` com status (ainda sem change) e, para a opção CPU na barra, a temperatura da CPU (`waterforce-cpu-feed`).
