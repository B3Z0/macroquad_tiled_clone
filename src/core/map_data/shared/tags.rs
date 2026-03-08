use crate::ir_map::IrObject;

pub(crate) fn object_has_tag(obj: &IrObject, tag: &str) -> bool {
    if let Some(v) = obj.properties.get_string("tag") {
        if v == tag {
            return true;
        }
    }
    if let Some(v) = obj.properties.get_string("tags") {
        return v.split(',').any(|t| t.trim() == tag);
    }
    false
}
