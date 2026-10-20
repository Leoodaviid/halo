# Spec Delta

## Purpose

Localizar os controladores RGB suportados pelo halo de forma estável entre boots, identificando cada um pelo VID:PID e pela usage page HID, e explicar ao usuário por que um dispositivo não pôde ser aberto.

## ADDED Requirements

### Requirement: Identificação por VID:PID e usage page
O sistema SHALL identificar cada controlador suportado pela combinação de VID:PID e usage page HID. O sistema MUST NOT depender de um caminho `/dev/hidrawN` fixo, já que esse número muda entre boots.

#### Scenario: Placa IT5702 encontrada em outro hidraw
- **WHEN** o controlador `048d:5702` com usage page `0xFF89` está conectado, em qualquer `/dev/hidrawN`
- **THEN** o sistema encontra o dispositivo e informa o caminho hidraw em que ele está

#### Scenario: Interface com usage page diferente
- **WHEN** existe uma interface HID com VID:PID `048d:5702` mas com usage page diferente de `0xFF89`
- **THEN** o sistema não a trata como o controlador RGB

### Requirement: Dispositivo ausente
O sistema SHALL informar com uma mensagem clara e um código de saída diferente de zero quando nenhum dispositivo suportado for encontrado, sem enviar nenhum comando.

#### Scenario: Nenhum controlador conectado
- **WHEN** o usuário pede um probe e não há dispositivo `048d:5702` com usage page `0xFF89`
- **THEN** o sistema informa que o controlador não foi encontrado e termina com código diferente de zero

### Requirement: Falta de permissão
O sistema SHALL distinguir falta de permissão de dispositivo ausente e indicar o caminho hidraw que não pôde ser aberto.

#### Scenario: hidraw sem permissão de leitura e escrita
- **WHEN** o dispositivo é encontrado mas o usuário não tem permissão para abrir o hidraw dele
- **THEN** o sistema informa que falta permissão, mostra o caminho e termina com código diferente de zero

### Requirement: Escopo de dispositivos abertos
O sistema MUST abrir apenas o dispositivo que a operação pedida precisa. Enumerar dispositivos SHALL usar só metadados (sysfs ou descritores) e MUST NOT abrir nem enviar dados a outros dispositivos.

#### Scenario: Probe da placa com o teclado conectado
- **WHEN** o usuário pede o probe do IT5702 e o teclado Redragon `258a:013b` também está conectado
- **THEN** o sistema não abre nem envia nada ao teclado
