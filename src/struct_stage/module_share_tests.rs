use super::*;
use crate::owned_str_runtime::event_trace_balanced;
const MODULE: &str = include_str!("../../tests/fixtures/p20/模組共享字串.voil");

#[test]
fn same_module_calls_observe_the_latest_value() {
    inspect(MODULE).unwrap();
    let (result, events) = Program::prepare(MODULE).unwrap().execute().unwrap();
    assert_eq!(result, Ok(90));
    use ResourceEvent::*;
    assert_eq!(
        events,
        [
            Created(1),
            Created(2),
            Released(1),
            Created(3),
            Released(2),
            Released(3)
        ]
    );
}

#[test]
fn declarations_enforce_scope_type_and_initialization() {
    for declaration in [
        "share x = \"x\"",
        "share x:str",
        "share x:int = 1",
        "share x:str = 3",
        "share x:str = \"a\"\nshare x:str = \"b\"",
        "share main:str = \"x\"",
    ] {
        assert!(inspect(&format!("{declaration}\nfn main()->int{{\nreturn 0\n}}")).is_err());
    }
    assert!(inspect("fn main()->int{\nshare x:str = \"x\"\nreturn 0\n}").is_err());
}

fn run(statements: &str) -> Result<(Result<i64, StrRuntimeError>, Vec<ResourceEvent>), Diagnostic> {
    let source = format!("share x:str = \"A\"\nfn set()->int{{\nx = \"Z\"\nreturn 0\n}}\nfn indirect()->int{{\nreturn set()\n}}\nfn main()->int{{\n{statements}\n}}");
    Ok(Program::prepare(&source)?.execute().unwrap())
}

#[test]
fn last_use_and_transitive_callee_effects() {
    for write in ["x = \"Z\"", "const k:int = indirect()"] {
        let result = run(&format!(
            "const a:str = x.borrow()\n{write}\nreturn a[0][0]"
        ));
        assert!(result.unwrap_err().message.contains("有效借用"));
    }
    let (v, _) = run(
        "const a:str = x.borrow()\nconst k:int = a[0][0]\nconst j:int = indirect()\nreturn x[0][0]",
    )
    .unwrap();
    assert_eq!(v, Ok(90));
}

#[test]
fn await_respects_borrows_in_ancestors_and_last_use() {
    let prefix = "share x:str = \"A\"\nasync fn job()->int{\nreturn 7\n}\nfn wrapper()->int{\nreturn await job()\n}\n";
    for call in ["await job()", "wrapper()"] {
        let source = format!("{prefix}fn main()->int{{\nconst a:str = x.borrow()\nconst k:int = {call}\nreturn a[0][0]\n}}");
        assert!(inspect(&source).unwrap_err().message.contains("VB15-01-A"));
    }
    let source = format!("{prefix}fn main()->int{{\nconst a:str = x.borrow()\nconst k:int = a[0][0]\nreturn await job()\n}}");
    assert_eq!(
        Program::prepare(&source).unwrap().execute().unwrap().0,
        Ok(7)
    );
    assert!(inspect(&source.replace("return await job()", "return job()")).is_err());
}

#[test]
fn alias_permissions() {
    assert!(run("const a:str = x.borrow()\na = \"Q\"\nreturn 1").is_err());
    assert!(
        run("const a:str = x.borrow()\nvar b:str = x.borrow()\nreturn a[0][0]+b[0][0]").is_err()
    );
    let (v, _) =
        run("const a:str = x.borrow()\nconst b:str = x.borrow()\nreturn a[0][0]+b[0][0]").unwrap();
    assert_eq!(v, Ok(130));
}

#[test]
fn exclusive_alias_and_unicode_replacement_use_the_original_owner() {
    let (value, events) = run("var a:str = x.borrow()\na[0] = \"界\"\nreturn x[0][0]").unwrap();
    assert_eq!(value, Ok(231));
    assert!(event_trace_balanced(&events));
    assert!(run("const a:str = x.borrow()\na[0] = \"Z\"\nreturn 0").is_err());
    let (value, events) = run("var a:str = x.borrow()\nreturn a[7][0]").unwrap();
    assert_eq!(value, Err(StrRuntimeError::EvaluationFailed));
    assert!(event_trace_balanced(&events));
}

#[test]
fn failed_rhs_preserves_storage_and_cleans_up() {
    let (v, events) = run("x = x[2]\nreturn 0").unwrap();
    assert_eq!(v, Err(StrRuntimeError::EvaluationFailed));
    assert_eq!(
        events,
        [ResourceEvent::Created(1), ResourceEvent::Released(1)]
    );
    let source = "share x:str = \"before\"\nfn main()->int{\nx = x[99]\nreturn 0\n}";
    let plan = Program::prepare(source).unwrap();
    let mut state = plan.instantiate(ModuleId(0)).unwrap();
    let original = state.heap.resource_id("x").unwrap();
    assert_eq!(
        plan.call("main", &mut state, 0),
        Err(StrRuntimeError::EvaluationFailed)
    );
    assert_eq!(state.heap.resource_id("x"), Ok(original));
    assert_eq!(state.heap.read("x"), Ok("before"));
    let mut heap = state.heap;
    let id = heap.resource_id("x").unwrap();
    assert!(heap.assign_checked("x", "after", true).is_err());
    assert_eq!(heap.read("x"), Ok("before"));
    assert_eq!(heap.resource_id("x"), Ok(id));
    assert!(event_trace_balanced(&heap.finish().unwrap()));
}

#[test]
fn external_read_requires_visibility_and_external_write_is_rejected() {
    let module = Program::prepare("share x:str = \"A\"\nfn main()->int{\nreturn 0\n}").unwrap();
    let other = Program::prepare("share x:str = \"B\"\nfn main()->int{\nreturn 0\n}").unwrap();
    let mut state = module.instantiate(ModuleId(1)).unwrap();
    let peer = other.instantiate(ModuleId(2)).unwrap();
    let remote = peer.owner;
    assert_eq!(peer.byte(remote, true, "x", 0, 0), Ok(66));
    assert_eq!(state.byte(remote, true, "x", 0, 0), Ok(65));
    assert!(state.byte(remote, false, "x", 0, 0).is_err());
    assert!(state.write(remote, "x", "Z").is_err());
    let loan = state
        .loans
        .acquire(module.globals[0].id, BorrowKind::ExclusiveWrite)
        .unwrap();
    assert!(state.byte(remote, true, "x", 0, 0).is_err());
    state.loans.release(loan).unwrap();
    assert_eq!(state.byte(state.owner, true, "x", 0, 0), Ok(65));
}
