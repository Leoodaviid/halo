# waterforce-telemetry-sanity

Temperatura do líquido (byte 0x0D do 99 DA): o firmware já reportou 224/240 °C (0xe0/0xf0) até um corte de energia. Só leitura: detectar o valor-sentinela e marcar a leitura como inválida na telemetria. Ver docs/hardware.md, 'Bomba plana e temperatura errada no DA'.
