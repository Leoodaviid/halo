// SPDX-License-Identifier: GPL-2.0-or-later

//! Leitura mínima de `uevent` e de descritores de report HID.

/// Barramento, VID e PID do campo `HID_ID` do `uevent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HidId {
    pub bus: u16,
    pub vendor: u16,
    pub product: u16,
}

/// Extrai o `HID_ID` de um `uevent`, no formato `HID_ID=0003:0000048D:00005702`.
pub fn parse_hid_id(uevent: &str) -> Option<HidId> {
    let value = uevent
        .lines()
        .find_map(|line| line.strip_prefix("HID_ID="))?;
    let mut parts = value.trim().split(':');
    let bus = u32::from_str_radix(parts.next()?, 16).ok()?;
    let vendor = u32::from_str_radix(parts.next()?, 16).ok()?;
    let product = u32::from_str_radix(parts.next()?, 16).ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(HidId {
        bus: u16::try_from(bus).ok()?,
        vendor: u16::try_from(vendor).ok()?,
        product: u16::try_from(product).ok()?,
    })
}

/// Extrai o `HID_NAME` de um `uevent`, se existir e não for vazio.
pub fn parse_hid_name(uevent: &str) -> Option<String> {
    let name = uevent
        .lines()
        .find_map(|line| line.strip_prefix("HID_NAME="))?
        .trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Usage Page em vigor quando cada coleção de topo abre, na ordem do descritor.
///
/// Não é um parser HID completo: só acompanha a Usage Page global e a
/// profundidade de Collection/End Collection. Itens longos são pulados.
pub fn top_level_usage_pages(descriptor: &[u8]) -> Vec<u16> {
    const LONG_ITEM: u8 = 0xFE;
    const USAGE_PAGE: u8 = 0x04; // tipo global, tag 0
    const COLLECTION: u8 = 0xA0; // tipo main, tag 0xA
    const END_COLLECTION: u8 = 0xC0; // tipo main, tag 0xC

    let mut pages = Vec::new();
    let mut usage_page: u16 = 0;
    let mut depth: usize = 0;
    let mut i = 0;

    while i < descriptor.len() {
        let prefix = descriptor[i];

        if prefix == LONG_ITEM {
            let Some(&size) = descriptor.get(i + 1) else {
                break;
            };
            i += 3 + usize::from(size);
            continue;
        }

        let size = match prefix & 0x03 {
            3 => 4,
            n => usize::from(n),
        };
        let Some(data) = descriptor.get(i + 1..i + 1 + size) else {
            break;
        };
        let value = data
            .iter()
            .rev()
            .fold(0u32, |acc, &b| (acc << 8) | u32::from(b));

        match prefix & 0xFC {
            USAGE_PAGE => usage_page = value as u16,
            COLLECTION => {
                if depth == 0 {
                    pages.push(usage_page);
                }
                depth += 1;
            }
            END_COLLECTION => depth = depth.saturating_sub(1),
            _ => {}
        }

        i += 1 + size;
    }

    pages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    // Descritores reais lidos de /sys/class/hidraw/*/device/report_descriptor.
    const IT5702: &str = "0689ff0910a101855a0901150026ff0075089510b100c00689ff09cca10185cc0901150026ff007508953fb100c0";
    const CASTOR3: &str =
        "05000902a101859905000902a1000900150025ff750896ff0081020900150025ff750896ff179102c0c0";

    #[test]
    fn hid_id_do_it5702() {
        let uevent = "DRIVER=hid-generic\nHID_ID=0003:0000048D:00005702\nHID_NAME=ITE Tech. Inc. ITE Device\n";
        assert_eq!(
            parse_hid_id(uevent),
            Some(HidId {
                bus: 0x0003,
                vendor: 0x048D,
                product: 0x5702
            })
        );
    }

    #[test]
    fn hid_name() {
        let uevent = "HID_ID=0003:00001044:00007A4D\nHID_NAME=ITE Tech. Castor3\n";
        assert_eq!(parse_hid_name(uevent).as_deref(), Some("ITE Tech. Castor3"));
        assert_eq!(parse_hid_name("HID_NAME=\n"), None);
        assert_eq!(parse_hid_name("HID_ID=0003:00001044:00007A4D\n"), None);
    }

    #[test]
    fn hid_id_ausente_ou_invalido() {
        assert_eq!(parse_hid_id("DRIVER=hid-generic\n"), None);
        assert_eq!(parse_hid_id("HID_ID=0003:0000048D\n"), None);
        assert_eq!(parse_hid_id("HID_ID=0003:XYZ:00005702\n"), None);
        assert_eq!(parse_hid_id("HID_ID=0003:0001048D:00005702\n"), None);
    }

    #[test]
    fn usage_pages_do_it5702() {
        assert_eq!(top_level_usage_pages(&hex(IT5702)), vec![0xFF89, 0xFF89]);
    }

    #[test]
    fn usage_pages_do_castor3() {
        // Tem uma coleção aninhada, que não conta como coleção de topo.
        assert_eq!(top_level_usage_pages(&hex(CASTOR3)), vec![0x0000]);
    }

    #[test]
    fn pagina_de_fabricante_depois_de_outras_colecoes() {
        // Desktop (0x01), depois Consumer (0x0C), depois fabricante (0xFF00),
        // como a interface 1 do teclado.
        let d = hex("05010980a10185018100c0050c0901a10185028100c00600ff0901a10185058100c0");
        assert_eq!(top_level_usage_pages(&d), vec![0x0001, 0x000C, 0xFF00]);
    }

    #[test]
    fn descritor_truncado_nao_entra_em_panico() {
        assert_eq!(top_level_usage_pages(&hex("0689")), Vec::<u16>::new());
        assert_eq!(top_level_usage_pages(&hex("0689ffa1")), Vec::<u16>::new());
        assert_eq!(top_level_usage_pages(&[0xFE]), Vec::<u16>::new());
    }
}
