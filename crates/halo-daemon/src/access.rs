// SPDX-License-Identifier: GPL-2.0-or-later

//! De onde vem o acesso do `halod` ao nó do Castor3 (`halod-outside-session`):
//! do grupo dono do nó (a regra udev com `GROUP="plugdev"`), que continua sem
//! sessão ativa, ou só da ACL que o logind dá à sessão (`uaccess`), que some
//! no logout.
//!
//! Com ACL estendida, os bits de grupo do `st_mode` são a máscara, e não o
//! grupo dono: a decisão lê `system.posix_acl_access` e exige `rw` em
//! `ACL_GROUP_OBJ` combinada com `ACL_MASK`. Entradas nominais (a do logind,
//! por exemplo) não contam. Qualquer caso não comprovado conta como "só ACL",
//! que é o lado que protege.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Nome do xattr da ACL de acesso no Linux.
const ACL_XATTR: &std::ffi::CStr = c"system.posix_acl_access";
/// Versão do formato do xattr (`POSIX_ACL_XATTR_VERSION`).
const ACL_VERSION: u32 = 2;
/// Cabeçalho (versão) e tamanho de cada entrada (`tag`, `perm`, `id`).
const ACL_HEADER: usize = 4;
const ACL_ENTRY: usize = 8;

const ACL_USER_OBJ: u16 = 0x01;
const ACL_USER: u16 = 0x02;
const ACL_GROUP_OBJ: u16 = 0x04;
const ACL_GROUP: u16 = 0x08;
const ACL_MASK: u16 = 0x10;
const ACL_OTHER: u16 = 0x20;

/// Leitura e escrita, nos bits de permissão de uma entrada ou do grupo do
/// `st_mode` deslocado.
const RW: u32 = 0o6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Access {
    /// O grupo dono (`gid`) tem leitura e escrita efetivas, e o processo é
    /// desse grupo: o acesso continua sem sessão ativa.
    Group(u32),
    /// O acesso depende da ACL da sessão, ou não deu para comprovar o grupo;
    /// o texto diz o porquê.
    AclOnly(String),
}

impl Access {
    /// A frase do log do daemon.
    pub fn describe(&self) -> String {
        match self {
            Self::Group(gid) => format!("acesso pelo grupo {}", group_name(*gid)),
            Self::AclOnly(why) => format!("acesso só pela ACL da sessão ({why})"),
        }
    }
}

/// Classifica o acesso a `node` agora. Refeito a cada chamada, porque a
/// regra udev pode mudar com o daemon rodando.
pub fn classify(node: &Path) -> Access {
    let meta = match std::fs::metadata(node) {
        Ok(m) => m,
        Err(e) => return Access::AclOnly(format!("stat de {}: {e}", node.display())),
    };
    decide(meta.mode(), meta.gid(), read_acl(node), &process_groups())
}

/// A decisão sobre os fatos do nó: o `st_mode`, o grupo dono, o xattr da ACL
/// (`Ok(None)` sem ACL estendida) e os grupos do processo.
pub fn decide(mode: u32, gid: u32, acl: Result<Option<Vec<u8>>, String>, groups: &[u32]) -> Access {
    if !groups.contains(&gid) {
        return Access::AclOnly(format!(
            "o processo não é do grupo dono do nó ({})",
            group_name(gid)
        ));
    }
    let perm = match acl {
        Err(why) => return Access::AclOnly(why),
        Ok(None) => (mode >> 3) & 0o7,
        Ok(Some(xattr)) => match effective_group(&xattr) {
            Ok(perm) => perm,
            Err(why) => return Access::AclOnly(why),
        },
    };
    if perm & RW == RW {
        Access::Group(gid)
    } else {
        Access::AclOnly(format!(
            "o grupo dono {} não tem leitura e escrita efetivas",
            group_name(gid)
        ))
    }
}

/// Permissão efetiva do grupo dono numa ACL no formato do xattr:
/// `ACL_GROUP_OBJ` combinada (AND) com a `ACL_MASK`, quando ela existe.
fn effective_group(xattr: &[u8]) -> Result<u32, String> {
    if xattr.len() < ACL_HEADER || !(xattr.len() - ACL_HEADER).is_multiple_of(ACL_ENTRY) {
        return Err(format!("ACL com {} bytes, formato inesperado", xattr.len()));
    }
    let version = u32::from_le_bytes(xattr[..4].try_into().unwrap());
    if version != ACL_VERSION {
        return Err(format!("ACL na versão {version}, formato inesperado"));
    }
    let mut group_obj = None;
    let mut mask = None;
    let mut named = false;
    for entry in xattr[ACL_HEADER..].as_chunks::<ACL_ENTRY>().0 {
        let tag = u16::from_le_bytes([entry[0], entry[1]]);
        let perm = u32::from(u16::from_le_bytes([entry[2], entry[3]]));
        let slot = match tag {
            ACL_GROUP_OBJ => &mut group_obj,
            ACL_MASK => &mut mask,
            ACL_USER | ACL_GROUP => {
                named = true;
                continue;
            }
            ACL_USER_OBJ | ACL_OTHER => continue,
            _ => return Err(format!("ACL com a entrada {tag:#x}, formato inesperado")),
        };
        if slot.replace(perm).is_some() {
            return Err("ACL com entrada repetida, formato inesperado".to_owned());
        }
    }
    let Some(group_obj) = group_obj else {
        return Err("ACL sem a entrada do grupo dono".to_owned());
    };
    match mask {
        Some(mask) => Ok(group_obj & mask),
        // Uma ACL com entradas nominais sempre tem máscara.
        None if named => Err("ACL com entradas nominais e sem máscara".to_owned()),
        None => Ok(group_obj),
    }
}

/// O xattr da ACL de `node`; `Ok(None)` só quando ele não existe.
fn read_acl(node: &Path) -> Result<Option<Vec<u8>>, String> {
    let path = CString::new(node.as_os_str().as_bytes())
        .map_err(|_| format!("caminho com NUL: {}", node.display()))?;
    let mut buf = vec![0u8; 1024];
    // SAFETY: `path` e `ACL_XATTR` terminam em NUL, e `buf` tem `buf.len()`
    // bytes graváveis durante a chamada.
    let n = unsafe {
        libc::getxattr(
            path.as_ptr(),
            ACL_XATTR.as_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    if n < 0 {
        let e = std::io::Error::last_os_error();
        return match e.raw_os_error() {
            Some(libc::ENODATA) => Ok(None),
            _ => Err(format!("ACL de {} ilegível: {e}", node.display())),
        };
    }
    buf.truncate(n as usize);
    Ok(Some(buf))
}

/// O grupo principal e os suplementares do processo.
fn process_groups() -> Vec<u32> {
    // SAFETY: `getgid` não falha. Com tamanho 0, `getgroups` só conta.
    let mut groups = vec![unsafe { libc::getgid() }];
    let n = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
    if n > 0 {
        let mut more = vec![0 as libc::gid_t; n as usize];
        // SAFETY: `more` tem `n` posições graváveis.
        let n = unsafe { libc::getgroups(n, more.as_mut_ptr()) };
        if n > 0 {
            more.truncate(n as usize);
            groups.extend(more);
        }
    }
    groups
}

/// O nome do grupo `gid`, ou o número quando não há nome.
fn group_name(gid: u32) -> String {
    let mut group: libc::group = unsafe { std::mem::zeroed() };
    let mut found = std::ptr::null_mut();
    let mut buf = vec![0 as libc::c_char; 4096];
    // SAFETY: `group`, `buf` e `found` são válidos durante a chamada; o nome
    // aponta para dentro de `buf`, lido antes de ele sair de escopo.
    let ok = unsafe { libc::getgrgid_r(gid, &mut group, buf.as_mut_ptr(), buf.len(), &mut found) };
    if ok != 0 || found.is_null() {
        return gid.to_string();
    }
    // SAFETY: com `found` não nulo, `gr_name` é uma string C dentro de `buf`.
    unsafe { std::ffi::CStr::from_ptr(group.gr_name) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLUGDEV: u32 = 46;
    const ME: u32 = 1000;

    /// xattr montado com as entradas `(tag, perm, id)`.
    fn acl(entries: &[(u16, u16, u32)]) -> Vec<u8> {
        let mut x = ACL_VERSION.to_le_bytes().to_vec();
        for &(tag, perm, id) in entries {
            x.extend(tag.to_le_bytes());
            x.extend(perm.to_le_bytes());
            x.extend(id.to_le_bytes());
        }
        x
    }

    const UNDEFINED: u32 = u32::MAX;

    /// `user::rw-`, `group::<group>`, `other::---`, com as entradas extras.
    fn node_acl(group: u16, extra: &[(u16, u16, u32)]) -> Vec<u8> {
        let mut entries = vec![
            (ACL_USER_OBJ, 6, UNDEFINED),
            (ACL_GROUP_OBJ, group, UNDEFINED),
        ];
        entries.extend_from_slice(extra);
        entries.push((ACL_OTHER, 0, UNDEFINED));
        acl(&entries)
    }

    fn is_acl_only(a: &Access) -> bool {
        matches!(a, Access::AclOnly(_))
    }

    #[test]
    fn sem_acl_com_o_processo_no_grupo() {
        // 0660 root:plugdev, sem ACL estendida.
        let a = decide(0o20660, PLUGDEV, Ok(None), &[ME, PLUGDEV]);
        assert_eq!(a, Access::Group(PLUGDEV));
    }

    #[test]
    fn sem_acl_grupo_sem_escrita() {
        let a = decide(0o20640, PLUGDEV, Ok(None), &[ME, PLUGDEV]);
        assert!(is_acl_only(&a), "{a:?}");
    }

    #[test]
    fn mascara_da_acl_nao_e_o_grupo() {
        // O caso do achado: `group::r--`, `user:<u>:rw-`, `mask::rw-`. Os
        // bits de grupo do `st_mode` (a máscara) dizem rw, mas o grupo dono
        // só lê.
        let x = node_acl(4, &[(ACL_USER, 6, ME), (ACL_MASK, 6, UNDEFINED)]);
        let a = decide(0o20660, PLUGDEV, Ok(Some(x)), &[ME, PLUGDEV]);
        assert!(is_acl_only(&a), "{a:?}");
    }

    #[test]
    fn acl_com_o_grupo_rw_e_mascara_rw() {
        let x = node_acl(6, &[(ACL_USER, 6, ME), (ACL_MASK, 6, UNDEFINED)]);
        let a = decide(0o20660, PLUGDEV, Ok(Some(x)), &[ME, PLUGDEV]);
        assert_eq!(a, Access::Group(PLUGDEV));
    }

    #[test]
    fn acl_com_a_mascara_so_leitura() {
        let x = node_acl(6, &[(ACL_USER, 6, ME), (ACL_MASK, 4, UNDEFINED)]);
        let a = decide(0o20640, PLUGDEV, Ok(Some(x)), &[ME, PLUGDEV]);
        assert!(is_acl_only(&a), "{a:?}");
    }

    #[test]
    fn grupo_nominal_nao_conta() {
        // `group:plugdev:rw-` nominal, com o grupo dono `root` sem acesso.
        let x = node_acl(0, &[(ACL_GROUP, 6, PLUGDEV), (ACL_MASK, 6, UNDEFINED)]);
        let a = decide(0o20660, 0, Ok(Some(x)), &[ME, PLUGDEV]);
        assert!(is_acl_only(&a), "{a:?}");
    }

    #[test]
    fn grupo_que_o_processo_nao_tem() {
        let a = decide(0o20660, PLUGDEV, Ok(None), &[ME]);
        assert!(is_acl_only(&a), "{a:?}");
        let x = node_acl(6, &[(ACL_MASK, 6, UNDEFINED)]);
        let a = decide(0o20660, PLUGDEV, Ok(Some(x)), &[ME]);
        assert!(is_acl_only(&a), "{a:?}");
    }

    #[test]
    fn xattr_truncado_ou_estranho() {
        let full = node_acl(6, &[(ACL_USER, 6, ME), (ACL_MASK, 6, UNDEFINED)]);
        let groups = [ME, PLUGDEV];
        for bad in [
            full[..full.len() - 3].to_vec(),
            full[..2].to_vec(),
            Vec::new(),
            // Versão desconhecida.
            [&3u32.to_le_bytes()[..], &full[4..]].concat(),
            // Entrada desconhecida.
            node_acl(6, &[(0x40, 6, 0), (ACL_MASK, 6, UNDEFINED)]),
            // Nominal sem máscara.
            node_acl(6, &[(ACL_USER, 6, ME)]),
            // Duas máscaras.
            node_acl(6, &[(ACL_MASK, 6, UNDEFINED), (ACL_MASK, 6, UNDEFINED)]),
            // Sem o grupo dono.
            acl(&[(ACL_USER_OBJ, 6, UNDEFINED), (ACL_OTHER, 0, UNDEFINED)]),
        ] {
            let a = decide(0o20660, PLUGDEV, Ok(Some(bad.clone())), &groups);
            assert!(is_acl_only(&a), "{bad:?}: {a:?}");
        }
        let a = decide(0o20660, PLUGDEV, Err("ilegível".into()), &groups);
        assert!(is_acl_only(&a));
    }

    #[test]
    fn acl_minima_sem_mascara() {
        let x = node_acl(6, &[]);
        assert_eq!(
            decide(0o20660, PLUGDEV, Ok(Some(x)), &[PLUGDEV]),
            Access::Group(PLUGDEV)
        );
    }

    #[test]
    fn no_inexistente_conta_como_so_acl() {
        let a = classify(Path::new("/caminho/que/nao/existe/hidraw250"));
        assert!(is_acl_only(&a), "{a:?}");
    }

    #[test]
    fn arquivo_comum_sem_acl_pelo_grupo_do_processo() {
        // Um arquivo do teste: grupo principal do processo, sem ACL.
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("no");
        std::fs::write(&f, b"").unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o660)).unwrap();
        assert!(matches!(classify(&f), Access::Group(_)));
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(is_acl_only(&classify(&f)));
    }
}
