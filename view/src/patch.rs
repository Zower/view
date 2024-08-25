use bevy_reflect::DynamicStruct;
use serde_json::Value;

#[macro_export]
macro_rules! patch {
    ($($tt:tt)+) => {
        crate::patch::into_dyn_struct(serde_json::json!({ $($tt)+ }))
    };
}

pub fn into_dyn_struct(value: Value) -> DynamicStruct {
    let Value::Object(object) = value else {
        unreachable!()
    };

    let mut stct = DynamicStruct::default();

    fn insert(k: String, v: Value, stct: &mut DynamicStruct) {
        match v {
            Value::Null => stct.insert(k, None::<()>),
            Value::Bool(val) => stct.insert(k, val),
            Value::Number(n) => {
                if n.is_f64() {
                    stct.insert(k, n.as_f64().unwrap())
                } else if n.is_u64() {
                    stct.insert(k, n.as_u64().unwrap())
                } else if n.is_i64() {
                    stct.insert(k, n.as_i64().unwrap())
                }
            }
            Value::String(str) => stct.insert(k, str),
            // Value::Array(arr) => for item in arr {
            //     stct.insert(k, )
            // },
            Value::Array(_) => todo!(),

            Value::Object(obj) => {
                for (k, v) in obj {
                    insert(k, v, stct)
                }
            }
        };
    }

    for (k, v) in object {
        insert(k, v, &mut stct)
        // stct.insert(k)
    }

    stct
}
