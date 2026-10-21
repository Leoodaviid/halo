# Tasks

## 1. Preparação

- [x] 1.1 Pedir ao usuário que instale o 7-Zip (`sudo apt install 7zip`) e verificar com `7z i | head -3`
- [x] 1.2 Extrair o NSIS do GCC em `tools/samples/gigabyte-control-center/extracted/nsis/` e o do RGB Fusion em `tools/samples/rgb-fusion/extracted/nsis/` com `7z x`, sem executar nada, e verificar listando os arquivos e registrando o sha256 de cada camada
- [x] 1.3 Extrair instaladores aninhados (MSI, NSIS, CAB) que aparecerem, cada um numa subpasta vazia nova, e verificar que nenhum arquivo ficou fora de `extracted/`

## 2. Triagem com o REA

- [x] 2.1 Rodar `inspect_desktop_application` nas árvores extraídas e registrar quais módulos são .NET, Electron ou nativos
- [x] 2.2 Buscar `7A4D`, `1044`, `Castor`, `WATERFORCE` e os bytes de comando conhecidos para localizar os módulos de cooler do GCC e do RGB Fusion, e registrar os caminhos e hashes encontrados
- [x] 2.3 Responder qual programa reconhece o `1044:7a4d` e com que nome de modelo, e atualizar a pergunta em aberto do `docs/roadmap.md` com a resposta e a fonte

## 3. Protocolo

- [x] 3.1 Mapear a identificação e o status (`D6`, `DA`, `DE` e outros) e conferir contra as respostas reais registradas em `docs/hardware.md`
- [x] 3.2 Mapear cor e efeito do anel e comparar com o protocolo do OpenRGB (`c9`, `cd`, `b0`–`b3`, `b6`), registrando o que confere e o que difere
- [x] 3.3 Mapear o RGB das fans (se é o mesmo canal do anel ou um canal separado)
- [x] 3.4 Mapear o controle de velocidade da fan e da bomba (modos, curvas, valores fixos)
- [x] 3.5 Mapear o envio de imagens para a LCD (resolução, formato de pixel, fragmentação em reports de 6144 bytes, comandos de início e fim)
- [x] 3.6 Se algum módulo necessário for nativo, parar e perguntar ao usuário sobre instalar o JDK 21+ e o Ghidra 12.1.x, registrando o ponto de parada

## 4. Documentação

- [x] 4.1 Escrever `docs/protocols/castor3.md` com cada comando, campos, fonte (`<dll>`, `<tipo>.<método>`, versão) e grau de confiança, e verificar que cada comando marcado como confirmado tem fonte
- [x] 4.2 Resumir em `docs/hardware.md` e atualizar a Fase 3 do `docs/roadmap.md` com as próximas changes de implementação, cada uma limitada a comandos confirmados

## Workflow follow-up

- Arquivar a change com `/opsx:archive` depois da revisão.
- Abrir as changes de implementação listadas na 4.2, cada uma começando por uma ferramenta que só lê, quando possível.
