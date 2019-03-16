use crate::{constants, wrappers, Ace, Trustee};
use std::fmt;
use std::io;
use std::mem;
use std::ptr::NonNull;
use winapi::shared::winerror::ERROR_INVALID_PARAMETER;
use winapi::um::winnt::ACL;

#[repr(C)]
pub struct Acl {
    inner: ACL,
}

impl Drop for Acl {
    fn drop(&mut self) {
        unreachable!("Acl should only be borrowed")
    }
}

impl Acl {
    /// Get a reference from an ACL pointer.
    ///
    /// ## Requirements
    ///
    /// - `ptr` must point to a valid ACL structure
    /// - The ACL header must be followed by the correct number of ACEs
    /// - The entire structure must remain alive at least as long as `'s`
    pub unsafe fn ref_from_nonnull<'s>(ptr: NonNull<ACL>) -> &'s Acl {
        let acl_ref: &Acl = mem::transmute(ptr);
        debug_assert!(wrappers::IsValidAcl(acl_ref));
        acl_ref
    }

    /// Get a pointer to the underlying ACL structure
    pub fn as_ptr(&self) -> *mut ACL {
        &self.inner as *const _ as *mut _
    }

    /// Determine what rights the given `Trustee` has under this ACL
    pub fn effective_rights(
        &self,
        trustee: &Trustee,
    ) -> Result<constants::AccessRights, io::Error> {
        wrappers::GetEffectiveRightsFromAcl(self, trustee)
    }

    /// Determine the number of ACEs in this ACL
    pub fn len(&self) -> u32 {
        wrappers::GetAclInformationSize(self)
            .expect("GetAclInformation failed on valid ACL")
            .AceCount
    }

    /// Get an ACE by index
    ///
    /// Returns `None` if there are too few ACEs to satisfy the request.
    pub fn get_ace(&self, index: u32) -> Option<&Ace> {
        match wrappers::GetAce(self, index) {
            Ok(ace) => Some(ace),
            Err(ref e) if e.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) => None,
            other_err => {
                other_err.expect("GetAce returned error on valid Ace");
                unreachable!() // Because other_err will always fail the expect
            }
        }
    }
}

impl fmt::Debug for Acl {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut map = fmt.debug_map();
        map.entry(&"len", &self.len());
        map.finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::constants::AceType;
    use crate::SecurityDescriptor;

    #[test]
    fn get_len() -> io::Result<()> {
        let limit = 100;

        for dacl_count in 0..limit {
            let sacl_count = limit - dacl_count - 1;

            // Looks like "D:(A;;;;;WD)(A;;;;;WD)(...)S:(AU;;;;;WD)(...)"
            // A (SDDL_ACCESS_ALLOWED) isn't valid for SACLs, AU (SDDL_AUDIT) is valid
            let mut sddl_string = String::new();
            sddl_string.push_str("D:");
            sddl_string.push_str(&"(A;;;;;WD)".repeat(dacl_count));
            sddl_string.push_str("S:");
            sddl_string.push_str(&"(AU;;;;;WD)".repeat(sacl_count));

            let sd: SecurityDescriptor = sddl_string.parse()?;

            assert_eq!(sd.dacl().unwrap().len(), dacl_count as u32);
            assert_eq!(sd.sacl().unwrap().len(), sacl_count as u32);
        }

        Ok(())
    }

    #[test]
    fn get_from_sddl() -> io::Result<()> {
        let mut sddl = "D:".to_string();
        let limit = 10;

        for i in 0..limit {
            sddl.push_str(&format!("(A;;;;;S-1-5-{})", i));
        }

        let sd: SecurityDescriptor = sddl.parse()?;
        let dacl = sd.dacl().unwrap();

        // Try to get each one
        for i in 0..limit {
            let ace = dacl.get_ace(i).unwrap();
            assert_eq!(ace.ace_type(), AceType::ACCESS_ALLOWED_ACE_TYPE);
        }

        // Off the end
        assert!(dacl.get_ace(limit).is_none());

        Ok(())
    }
}
