//! VA16-01/02/03-A：僅供 Rust 內部測試的真實 UTF-8 字串資源核心。
//! 單一持有者的 String 緩衝區透過 Option::take() 整筆轉交；沒有隱含複製。
//! 以資源身分和明示釋放事件驗證清理；Rust Drop 只負責實際釋放緩衝區，
//! voiles 的先後次序由本模型的完成初始化序號決定，而非宿主 Drop 推定。
//! P14/P15 完整來源借用驗證、結構預設值求值順序及 P17 執行入口仍待完成。
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StrRuntimeError {
    MissingOwner,
    DuplicateBinding,
    ImmutableBinding,
    MovedSource,
    NoInnerScope,
    IncompleteReturn,
    EvaluationFailed,
    ExhaustedIdentity,
    OutstandingResource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResourceEvent {
    Created(u64),
    Moved(u64),
    Released(u64),
    Returned(u64),
}

#[derive(Debug)]
pub(crate) struct OwnedUtf8 {
    id: u64,
    text: String,
}

#[derive(Debug)]
struct StrBinding {
    name: String,
    mutable: bool,
    initialized_at: u64,
    owned: Option<OwnedUtf8>,
}

#[derive(Debug, Default)]
struct StrFrame {
    names: Vec<StrBinding>,
}

#[derive(Debug)]
pub(crate) struct OwnedStrRuntime {
    scopes: Vec<StrFrame>,
    next_id: u64,
    next_order: u64,
    events: Vec<ResourceEvent>,
    created: usize,
    released: usize,
}

impl OwnedStrRuntime {
    pub(crate) fn new() -> Self {
        Self {
            scopes: vec![StrFrame::default()],
            next_id: 1,
            next_order: 1,
            events: Vec::new(),
            created: 0,
            released: 0,
        }
    }

    fn next_order(&mut self) -> Result<u64, StrRuntimeError> {
        let order = self.next_order;
        self.next_order = order.checked_add(1).ok_or(StrRuntimeError::ExhaustedIdentity)?;
        Ok(order)
    }

    fn allocate(&mut self, value: String) -> Result<OwnedUtf8, StrRuntimeError> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or(StrRuntimeError::ExhaustedIdentity)?;
        self.created += 1;
        self.events.push(ResourceEvent::Created(id));
        Ok(OwnedUtf8 {id, text: value})
    }

    fn release(&mut self, resource: OwnedUtf8) {
        self.released += 1;
        self.events.push(ResourceEvent::Released(resource.id));
        // 真正持有 String，所有權僅於此釋放，不需垃圾回收器。
        drop(resource);
    }

    fn location(&self, name: &str) -> Result<(usize, usize), StrRuntimeError> {
        self.scopes.iter().enumerate().rev().find_map(|(i, frame)| {
            frame.names.iter().position(|slot| slot.name == name).map(|j| (i,j))
        }).ok_or(StrRuntimeError::MissingOwner)
    }

    fn current_name_free(&self, name: &str) -> Result<(), StrRuntimeError> {
        if self.scopes.last().expect("根作用域存在").names.iter().any(|x| x.name == name) {
            Err(StrRuntimeError::DuplicateBinding)
        } else { Ok(()) }
    }

    fn insert(&mut self, name: &str, mutable: bool, value: OwnedUtf8)
        -> Result<(), StrRuntimeError> {
        let order = self.next_order()?;
        self.scopes.last_mut().expect("根作用域存在").names.push(StrBinding {
            name: name.to_owned(), mutable, initialized_at: order, owned: Some(value),
        });
        Ok(())
    }

    pub(crate) fn declare(&mut self, name: &str, mutable: bool, text: &str)
        -> Result<(), StrRuntimeError> {
        self.current_name_free(name)?;
        let order = self.next_order()?;
        let value = self.allocate(text.to_owned())?;
        self.scopes.last_mut().expect("根作用域存在").names.push(StrBinding {
            name: name.to_owned(), mutable, initialized_at: order, owned: Some(value),
        });
        Ok(())
    }

    pub(crate) fn read(&self, name: &str) -> Result<&str, StrRuntimeError> {
        let (i,j) = self.location(name)?;
        self.scopes[i].names[j].owned.as_ref()
            .map(|v| v.text.as_str()).ok_or(StrRuntimeError::MovedSource)
    }

    pub(crate) fn enter_scope(&mut self) { self.scopes.push(StrFrame::default()); }

    /// 明確 .move()：一個緩衝區只轉交，來源仍存在但已失效。
    pub(crate) fn move_named(&mut self, from: &str, to: &str)
        -> Result<(), StrRuntimeError> {
        self.current_name_free(to)?;
        let (i,j) = self.location(from)?;
        if self.scopes[i].names[j].owned.is_none() {
            return Err(StrRuntimeError::MovedSource);
        }
        // 在來源值被取走之前先完成所有可能失敗的驗證與序號配置。
        let order = self.next_order()?;
        let value = self.scopes[i].names[j].owned.take().expect("已驗證原來源有效");
        self.events.push(ResourceEvent::Moved(value.id));
        self.scopes.last_mut().expect("根作用域存在").names.push(StrBinding {
            name: to.to_owned(), mutable: true, initialized_at: order, owned: Some(value),
        });
        Ok(())
    }

    /// VA16-03-A：右側先成功取得字串；才把新所有者接到舊 var 綁定。
    /// 成功後由舊綁定負責清理原資源，不透過 String 複製。
    pub(crate) fn assign(&mut self, name: &str, right: &str)
        -> Result<(), StrRuntimeError> {
        self.assign_checked(name, right, false)
    }

    /// 模擬右側完成資源建立後的可測試失敗點；失敗只清理暫存的
    /// 新資源並保持原綁定完整，並非宣稱已完成 voiles 任意運算式。
    pub(crate) fn assign_checked(&mut self, name: &str, right: &str, fail_after_eval: bool)
        -> Result<(), StrRuntimeError> {
        let (i,j) = self.location(name)?;
        if !self.scopes[i].names[j].mutable {
            return Err(StrRuntimeError::ImmutableBinding);
        }
        let order = self.next_order()?;
        let pending = self.allocate(right.to_owned())?;
        if fail_after_eval {
            self.release(pending);
            return Err(StrRuntimeError::EvaluationFailed);
        }
        let previous = self.scopes[i].names[j].owned.replace(pending);
        self.scopes[i].names[j].initialized_at = order;
        if let Some(previous) = previous { self.release(previous); }
        Ok(())
    }

    /// 返回時原作用域不得釋放已轉交的緩衝區。
    pub(crate) fn return_named(&mut self, name: &str)
        -> Result<OwnedUtf8, StrRuntimeError> {
        let (i,j) = self.location(name)?;
        let value = self.scopes[i].names[j].owned.take()
            .ok_or(StrRuntimeError::MovedSource)?;
        self.events.push(ResourceEvent::Returned(value.id));
        Ok(value)
    }

    pub(crate) fn accept_return(&mut self, name: &str, result: OwnedUtf8)
        -> Result<(), (StrRuntimeError, OwnedUtf8)> {
        // 失敗時將原資源交還呼叫者，不能在未證明的地方暗中丟棄。
        if let Err(err) = self.current_name_free(name) { return Err((err, result)); }
        if self.next_order == u64::MAX {
            return Err((StrRuntimeError::ExhaustedIdentity, result));
        }
        let order = self.next_order;
        self.next_order += 1;
        self.scopes.last_mut().expect("根作用域存在").names.push(StrBinding {
            name: name.to_owned(), mutable: true, initialized_at: order, owned: Some(result),
        });
        Ok(())
    }

    fn cleanup_frame(&mut self, mut frame: StrFrame) {
        frame.names.sort_by_key(|slot| slot.initialized_at);
        for slot in frame.names.into_iter().rev() {
            if let Some(value) = slot.owned { self.release(value); }
        }
    }

    pub(crate) fn leave_scope(&mut self) -> Result<(), StrRuntimeError> {
        if self.scopes.len() == 1 { return Err(StrRuntimeError::NoInnerScope); }
        let frame = self.scopes.pop().expect("已確認內層");
        self.cleanup_frame(frame);
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<Vec<ResourceEvent>, StrRuntimeError> {
        if self.scopes.len() != 1 { return Err(StrRuntimeError::NoInnerScope); }
        let root = self.scopes.pop().expect("根作用域");
        self.cleanup_frame(root);
        if self.created != self.released {
            return Err(StrRuntimeError::OutstandingResource);
        }
        Ok(self.events)
    }

    pub(crate) fn events(&self) -> &[ResourceEvent] { &self.events }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn released(events: &[ResourceEvent]) -> Vec<u64> {
        events.iter().filter_map(|event| {
            if let ResourceEvent::Released(id) = event { Some(*id) } else { None }
        }).collect()
    }

    #[test]
    fn va16_utf8_緩衝區實際保存中文和表情字素() {
        let mut rt = OwnedStrRuntime::new();
        rt.declare("s", true, "繁體中文 🪼").unwrap();
        assert_eq!(rt.read("s"), Ok("繁體中文 🪼"));
        assert_eq!(released(&rt.finish().unwrap()), vec![1]);
    }

    #[test]
    fn va16_move_不複製字串緩衝區_來源失效且只清理一次() {
        let mut rt = OwnedStrRuntime::new();
        rt.declare("s", true, "資源").unwrap();
        rt.move_named("s","target").unwrap();
        assert_eq!(rt.read("s"), Err(StrRuntimeError::MovedSource));
        assert_eq!(rt.read("target"), Ok("資源"));
        let events=rt.finish().unwrap();
        assert_eq!(events.iter().filter(|e| **e == ResourceEvent::Created(1)).count(),1);
        assert_eq!(released(&events),vec![1]);
    }

    #[test]
    fn va16_scope_成功初始化相反順序清理() {
        let mut rt = OwnedStrRuntime::new();
        rt.declare("a",true,"A").unwrap();
        rt.enter_scope();
        rt.declare("b",true,"B").unwrap();
        rt.declare("c",true,"C").unwrap();
        rt.leave_scope().unwrap();
        assert_eq!(released(rt.events()), vec![3,2]);
        assert_eq!(released(&rt.finish().unwrap()),vec![3,2,1]);
    }

    #[test]
    fn va16_覆寫後以新的完成初始化時點反向清理() {
        let mut rt = OwnedStrRuntime::new();
        rt.declare("a",true,"old").unwrap();
        rt.declare("b",true,"B").unwrap();
        rt.assign("a","new").unwrap();
        assert_eq!(rt.read("a"),Ok("new"));
        assert_eq!(released(rt.events()),vec![1]);
        assert_eq!(released(&rt.finish().unwrap()),vec![1,3,2]);
    }

    #[test]
    fn va16_右值失敗保留舊值並清理僅建立的暫存資源() {
        let mut rt = OwnedStrRuntime::new();
        rt.declare("x",true,"original").unwrap();
        assert_eq!(rt.assign_checked("x","temporary",true),
            Err(StrRuntimeError::EvaluationFailed));
        assert_eq!(rt.read("x"),Ok("original"));
        assert_eq!(released(rt.events()),vec![2]);
        assert_eq!(released(&rt.finish().unwrap()),vec![2,1]);
    }

    #[test]
    fn va16_返回值轉交呼叫端_原作用域不能釋放() {
        let mut rt = OwnedStrRuntime::new();
        rt.enter_scope();
        rt.declare("local",true,"returned").unwrap();
        let value = rt.return_named("local").unwrap();
        rt.leave_scope().unwrap();
        assert_eq!(released(rt.events()),Vec::<u64>::new());
        rt.accept_return("received",value).unwrap();
        assert_eq!(rt.read("received"),Ok("returned"));
        assert_eq!(released(&rt.finish().unwrap()),vec![1]);
    }

    #[test]
    fn va16_重複名稱和_const_覆寫在建立新資源前被拒絕() {
        let mut rt = OwnedStrRuntime::new();
        rt.declare("a",false,"constant").unwrap();
        assert_eq!(rt.declare("a",true,"other"),Err(StrRuntimeError::DuplicateBinding));
        assert_eq!(rt.assign("a","changed"),Err(StrRuntimeError::ImmutableBinding));
        assert_eq!(rt.read("a"),Ok("constant"));
        assert_eq!(released(&rt.finish().unwrap()),vec![1]);
    }

    #[test]
    fn va16_已轉交資源絕不因離開原詞彙區塊提前釋放() {
        let mut rt = OwnedStrRuntime::new();
        rt.enter_scope();
        rt.declare("inside",true,"only owner").unwrap();
        rt.move_named("inside","moved").unwrap();
        // 原值雖已 moved，仍在相同子作用域；再移動至父作用域模擬返回。
        let value=rt.return_named("moved").unwrap();
        rt.leave_scope().unwrap();
        assert!(released(rt.events()).is_empty());
        rt.accept_return("outside",value).unwrap();
        assert_eq!(released(&rt.finish().unwrap()),vec![1]);
    }

    #[test]
    fn va16_若返回資源未交接必須偵測責任缺口() {
        let mut rt = OwnedStrRuntime::new();
        rt.declare("a",true,"escaped").unwrap();
        let escaped=rt.return_named("a").unwrap();
        assert!(matches!(rt.finish(),Err(StrRuntimeError::OutstandingResource)));
        // 此為故意不完整資源責任的負例：Rust 最後會釋放 escaped，
        // 但不能把宿主自動釋放誤認為 voiles 清理順序證明。
        drop(escaped);
    }

    #[test]
    fn va16_移動到相同範圍已存在名稱時原始所有權不損毀() {
        let mut rt = OwnedStrRuntime::new();
        rt.declare("a",true,"A").unwrap();
        rt.declare("b",true,"B").unwrap();
        assert_eq!(rt.move_named("a","b"),Err(StrRuntimeError::DuplicateBinding));
        assert_eq!(rt.read("a"),Ok("A"));
        assert_eq!(rt.read("b"),Ok("B"));
        assert_eq!(released(&rt.finish().unwrap()),vec![2,1]);
    }
}
