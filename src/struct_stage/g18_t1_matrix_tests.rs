#[cfg(test)]
mod g18_t1_matrix_tests {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent::{self as R, Created, Released};

    fn four(src:&str, value:i64, bad:(&str,&str), resources:&[R]) {
        let checked = PreparedPrivateScalarCall::prepare(src).unwrap();
        let ok = checked.execute().unwrap();
        assert_eq!(ok.result,Ok(value));
        assert_eq!(ok.resource_events,resources);
        assert!(ok.has_balanced_frame_trace());
        assert!(src.contains(bad.0));
        assert!(matches!(PreparedPrivateScalarCall::prepare(&src.replace(bad.0,bad.1)),
            Err(PrivateScalarCallPrepareError::Check(_))));
        let overflow = src.replace("var n:int = 2", "var n:int = 9223372036854775807");
        assert_ne!(overflow,src);
        let failed = PreparedPrivateScalarCall::prepare(&overflow).unwrap().execute().unwrap();
        assert_eq!(failed.result,Err(PrivateScalarCallRuntimeError::IntegerOverflow));
        assert_eq!(failed.resource_events,resources);
        assert!(failed.has_balanced_frame_trace());
        assert!(failed.events.iter().any(|e| matches!(e,PrivateScalarCallEvent::UnwindStarted)));
    }

    #[test]
    fn t1_f_普通呼叫四欄() {
        four(r#"fn leaf(x:int)->int{
var guard:str = "子"
return x+1
}
fn main()->int{
var root:str = "父"
var n:int = 2
return leaf(n)
}
"#,3,("leaf(n)","leaf(false)"),&[Created(1),Created(2),Released(2),Released(1)]);
    }
    #[test]
    fn t1_c_選中迴圈路徑四欄() {
        four(r#"fn work(x:int)->int{
var total:int = x
for i in 0..1{
var guard:str = "輪"
if(true){ total = total+1 }
}
return total
}
fn main()->int{
var n:int = 2
return work(n)
}
"#,3,("if(true)","if(x)"),&[Created(1),Released(1)]);
    }
    #[test]
    fn t1_r_遞迴內層溢位與逐層資源四欄() {
        four(r#"fn recur(flag:bool,x:int)->int{
var guard:str = "每層"
if(flag){ return recur(false,x) }
return x+1
}
fn main()->int{
var n:int = 2
return recur(true,n)
}
"#,3,("recur(false,x)","recur(x,false)"),&[Created(1),Created(2),Released(2),Released(1)]);
    }
    #[test]
    fn t1_b_借用形參正常與故障還票四欄() {
        four(r#"fn take(view:str,x:int)->int{ return x+1 }
fn main()->int{
const source:str = "借入"
var n:int = 2
return take(source,n)
}
"#,3,("take(source,n)","take(n,source)"),&[Created(1),Released(1)]);
    }
    #[test]
    fn t1_m_獨占方法動態退出四欄() {
        four(r#"struct Counter{
var v:int = 1
fn put(x:int)->int{
self.v = self.v+x
return self.v
}
}
fn main()->int{
var guard:str = "鄰接"
var h:Counter = Counter()
var n:int = 2
return h.put(n)
}
"#,3,("var h:Counter","const h:Counter"),&[Created(1),Released(1)]);
    }
    #[test]
    fn t1_a_直接等待方法與普通框架四欄() {
        four(r#"struct Counter{
var v:int = 1
async fn value(x:int)->int{ return self.v+x }
}
async fn work(x:int)->int{
var guard:str = "非同步"
var h:Counter = Counter()
return await h.value(x)
}
fn main()->int{
var n:int = 2
return await work(n)
}
"#,3,("await h.value(x)","h.value(x)"),&[Created(1),Released(1)]);
    }
}
