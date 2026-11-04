//! P18-E：由真正執行器產生的控制事件之結構完整性稽核。
//! 不推導靜態 CFG，不核准未實作的控制語法，不產生任何 ResourceEvent。

use crate::common_ir::NodeSpan;
use super::ControlEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlAuditError {
    UnmatchedFunctionExit,
    FunctionIdentityMismatch,
    DanglingFunctionFrame,
    EventOutsideFunction,
    EventOutsideSource,
    ReturnCountMismatch,
}

struct ActiveFunction<'a> {
    name: &'a str,
    span: NodeSpan,
    returns: usize,
}

fn contained(parent: NodeSpan, child: NodeSpan) -> bool {
    child.start < child.end
        && parent.start <= child.start
        && child.end <= parent.end
}

/// 僅由現存 Runtime 真發出的事件形成函式堆疊；
/// 成功離開的函式必須恰好實際執行一次 return，
/// 例外離開不能聲稱曾成功返回。
pub(crate) fn audit_control_events(events: &[ControlEvent])
    -> Result<(), ControlAuditError> {
    let mut frames: Vec<ActiveFunction<'_>> = Vec::new();
    for event in events {
        match event {
            ControlEvent::FunctionEntered { name, span } => {
                if span.start >= span.end {
                    return Err(ControlAuditError::EventOutsideSource);
                }
                frames.push(ActiveFunction { name, span: *span, returns: 0 });
            }
            ControlEvent::FunctionExited { name, span, returned } => {
                let current = frames.pop().ok_or(ControlAuditError::UnmatchedFunctionExit)?;
                if current.name != name || current.span != *span {
                    return Err(ControlAuditError::FunctionIdentityMismatch);
                }
                if current.returns != usize::from(*returned) {
                    return Err(ControlAuditError::ReturnCountMismatch);
                }
            }
            ControlEvent::Returned { span } => {
                let current = frames.last_mut().ok_or(ControlAuditError::EventOutsideFunction)?;
                if !contained(current.span, *span) {
                    return Err(ControlAuditError::EventOutsideSource);
                }
                current.returns += 1;
                if current.returns > 1 {
                    return Err(ControlAuditError::ReturnCountMismatch);
                }
            }
            ControlEvent::ConditionChecked { span, .. }
            | ControlEvent::ElseSelected { span } => {
                let current = frames.last().ok_or(ControlAuditError::EventOutsideFunction)?;
                if !contained(current.span, *span) {
                    return Err(ControlAuditError::EventOutsideSource);
                }
            }
        }
    }
    if frames.is_empty() { Ok(()) } else { Err(ControlAuditError::DanglingFunctionFrame) }
}


/// P18-E：執行時事件與正式來源 AST 的唯一合法位置對照。
/// 相較既有 audit_control_events 僅檢查堆疊及包含範圍，這裡要求
/// return、if 條件與 else 必須真正在 Parser 的同角色節點出現。
/// 不以字串搜尋或另一份 Parser 建立來源身分。
#[derive(Default)]
struct SourceSites {
    functions: std::collections::BTreeMap<NodeSpan, String>,
    returns: std::collections::BTreeMap<NodeSpan, NodeSpan>,
    conditions: std::collections::BTreeMap<NodeSpan, NodeSpan>,
    else_blocks: std::collections::BTreeMap<NodeSpan, NodeSpan>,
}

impl SourceSites {
    fn gather_function(
        &mut self,
        function: &crate::parser::Function,
        source: &str,
    ) -> Result<(), ControlAuditError> {
        let span = NodeSpan::new(function.start, function.end);
        if span.checked(source).is_none() || self.functions.insert(span, function.name.clone()).is_some() {
            return Err(ControlAuditError::EventOutsideSource);
        }
        self.gather_block(&function.body, span, source)
    }

    fn gather_block(
        &mut self,
        block: &crate::parser::Block,
        function: NodeSpan,
        source: &str,
    ) -> Result<(), ControlAuditError> {
        let block_span = NodeSpan::new(block.start, block.end);
        if block_span.checked(source).is_none() || !contained(function, block_span) {
            return Err(ControlAuditError::EventOutsideSource);
        }
        for statement in &block.statements {
            let span = statement.node_span();
            if span.checked(source).is_none() || !contained(block_span, span) {
                return Err(ControlAuditError::EventOutsideSource);
            }
            match &statement.kind {
                crate::parser::StmtKind::Return(expression) => {
                    if !contained(span, expression.expression.node_span())
                        || self.returns.insert(span, function).is_some() {
                        return Err(ControlAuditError::EventOutsideSource);
                    }
                }
                crate::parser::StmtKind::IfChain(chain) => {
                    for arm in &chain.arms {
                        let condition = arm.branch.condition.node_span();
                        if !contained(span, condition)
                            || self.conditions.insert(condition, function).is_some() {
                            return Err(ControlAuditError::EventOutsideSource);
                        }
                        self.gather_block(&arm.branch.body, function, source)?;
                    }
                    if let Some(otherwise) = &chain.otherwise {
                        let else_span = NodeSpan::new(otherwise.start, otherwise.end);
                        if !contained(span, else_span)
                            || self.else_blocks.insert(else_span, function).is_some() {
                            return Err(ControlAuditError::EventOutsideSource);
                        }
                        self.gather_block(otherwise, function, source)?;
                    }
                }
                crate::parser::StmtKind::Binding(_) |
                crate::parser::StmtKind::Assignment(_) => {}
            }
        }
        Ok(())
    }
}

/// 驗證事件必須由「這一棵」原本成功經 Lexer／Parser 的正式公開 AST
/// 產生；每一個 Runtime 控制節點都要與已驗證函式內的精確角色跨度吻合。
/// 完整控制語意仍由原 Checker 負責，這不是新的語言能力授權。
pub(crate) fn audit_control_events_against_source(
    events: &[ControlEvent],
    file: &crate::parser::File,
    source: &str,
) -> Result<(), ControlAuditError> {
    audit_control_events(events)?;
    if crate::parser::verified_public_syntax_roles(file, source).is_none() {
        return Err(ControlAuditError::EventOutsideSource);
    }
    let mut sites = SourceSites::default();
    for item in &file.items {
        if let crate::parser::Item::Function(function) = item {
            sites.gather_function(function, source)?;
        }
    }
    let mut stack: Vec<NodeSpan> = Vec::new();
    for event in events {
        match event {
            ControlEvent::FunctionEntered { name, span } => {
                if sites.functions.get(span).map(String::as_str) != Some(name) {
                    return Err(ControlAuditError::FunctionIdentityMismatch);
                }
                stack.push(*span);
            }
            ControlEvent::FunctionExited { span, .. } => {
                if stack.pop() != Some(*span) {
                    return Err(ControlAuditError::FunctionIdentityMismatch);
                }
            }
            ControlEvent::Returned { span } => {
                if sites.returns.get(span).copied() != stack.last().copied() {
                    return Err(ControlAuditError::EventOutsideSource);
                }
            }
            ControlEvent::ConditionChecked { span, .. } => {
                if sites.conditions.get(span).copied() != stack.last().copied() {
                    return Err(ControlAuditError::EventOutsideSource);
                }
            }
            ControlEvent::ElseSelected { span } => {
                if sites.else_blocks.get(span).copied() != stack.last().copied() {
                    return Err(ControlAuditError::EventOutsideSource);
                }
            }
        }
    }
    if stack.is_empty() { Ok(()) }
    else { Err(ControlAuditError::DanglingFunctionFrame) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn src() -> NodeSpan { NodeSpan::new(0, 100) }
    fn point() -> NodeSpan { NodeSpan::new(10, 20) }
    fn enter(name: &str) -> ControlEvent {
        ControlEvent::FunctionEntered { name: name.into(), span: src() }
    }
    fn exit(name: &str, returned: bool) -> ControlEvent {
        ControlEvent::FunctionExited { name: name.into(), span: src(), returned }
    }


    #[test]
    fn p18_真實_checker_來源的控制事件可通過_ast_角色核對() {
        let code = "fn helper()->int{\nreturn 2\n}\nfn main()->int{\nif (true){\nreturn helper()+1\n}\nreturn 0\n}\n";
        let prepared = super::PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let result = prepared.execute_with_trace();
        assert_eq!(result.result, Ok(super::Value::Int(3)));
        assert_eq!(audit_control_events_against_source(
            &result.events, &prepared.ast, code,
        ), Ok(()));
    }

    #[test]
    fn p18_相同函式內的合法跨度不能冒充另一個_return_角色() {
        let code = "fn main()->int{\nif (true){\nreturn 1\n}\nreturn 0\n}\n";
        let prepared = super::PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let mut events = prepared.execute_with_trace().events;
        let condition = events.iter().find_map(|event| match event {
            ControlEvent::ConditionChecked { span, .. } => Some(*span),
            _ => None,
        }).unwrap();
        for event in &mut events {
            if let ControlEvent::Returned { span } = event {
                *span = condition;
                break;
            }
        }
        // 原本的事件堆疊／包含範圍檢查不足以辨別角色錯置。
        assert_eq!(audit_control_events(&events), Ok(()));
        assert_eq!(audit_control_events_against_source(
            &events, &prepared.ast, code,
        ), Err(ControlAuditError::EventOutsideSource));
    }

    #[test]
    fn p18_同函式內非法_else_事件不得借用正常_span_冒充來源() {
        let code = "fn main()->int{\nif (true){\nreturn 1\n}\nreturn 0\n}\n";
        let prepared = super::PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let mut events = prepared.execute_with_trace().events;
        let condition = events.iter().find_map(|event| match event {
            ControlEvent::ConditionChecked { span, .. } => Some(*span),
            _ => None,
        }).unwrap();
        events.insert(2, ControlEvent::ElseSelected { span: condition });
        assert_eq!(audit_control_events(&events), Ok(()));
        assert_eq!(audit_control_events_against_source(
            &events, &prepared.ast, code,
        ), Err(ControlAuditError::EventOutsideSource));
    }

    #[test]
    fn p18_另一函式的真跨度不得冒用現行函式身分() {
        let code = "fn helper()->int{\nreturn 1\n}\nfn main()->int{\nreturn helper()\n}\n";
        let prepared = super::PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let mut events = prepared.execute_with_trace().events;
        let replacement = events.iter().find_map(|event| match event {
            ControlEvent::FunctionEntered { name, span } if name == "helper" => Some(*span),
            _ => None,
        }).unwrap();
        if let ControlEvent::FunctionEntered { span, .. } = &mut events[0] {
            *span = replacement;
        }
        // 此處堆疊稽核亦可能先偵測身分衝突，來源稽核必定拒絕。
        assert!(audit_control_events_against_source(
            &events, &prepared.ast, code,
        ).is_err());
    }

    #[test]
    fn p18_竄改非原始來源的_函式_ast_或文字不得授權執行事件() {
        let code = "fn main()->int{\nreturn 7\n}\n";
        let prepared = super::PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        let events = prepared.execute_with_trace().events;
        assert_eq!(audit_control_events_against_source(
            &events, &prepared.ast, code,
        ), Ok(()));
        let tampered = code.replacen("return 7", "return 8", 1);
        // 同長度編碼也不能獲得先前 Parser 的完整來源授權。
        // 目前 verified_public_syntax_roles 只驗角色與跨度，
        // 所以此一來源變更由 PreparedProgram 的位元組指紋再次守衛。
        let mut changed = super::PreparedProgram::prepare(code.as_bytes().to_vec()).unwrap();
        changed.source.bytes = tampered.into_bytes();
        assert!(changed.execute_with_trace().events.is_empty());
    }

    #[test]
    fn p18_真實控制事件稽核接受巢狀函式成對成功返回() {
        assert_eq!(audit_control_events(&[
            enter("main"), enter("helper"),
            ControlEvent::Returned { span: point() }, exit("helper", true),
            ControlEvent::Returned { span: point() }, exit("main", true),
        ]), Ok(()));
    }

    #[test]
    fn p18_控制事件稽核拒絕無進入就退出與漏退出() {
        assert_eq!(audit_control_events(&[exit("main", false)]),
            Err(ControlAuditError::UnmatchedFunctionExit));
        assert_eq!(audit_control_events(&[enter("main")]),
            Err(ControlAuditError::DanglingFunctionFrame));
    }

    #[test]
    fn p18_控制事件稽核拒絕未執行返回卻虛報成功或錯報失敗() {
        assert_eq!(audit_control_events(&[enter("main"), exit("main", true)]),
            Err(ControlAuditError::ReturnCountMismatch));
        assert_eq!(audit_control_events(&[
            enter("main"), ControlEvent::Returned { span: point() }, exit("main", false),
        ]), Err(ControlAuditError::ReturnCountMismatch));
    }

    #[test]
    fn p18_控制事件稽核拒絕假來源跨度與跨函式身分() {
        assert_eq!(audit_control_events(&[
            enter("main"), ControlEvent::Returned { span: NodeSpan::new(100, 110) },
            exit("main", true),
        ]), Err(ControlAuditError::EventOutsideSource));
        assert_eq!(audit_control_events(&[enter("main"),exit("other", false)]),
            Err(ControlAuditError::FunctionIdentityMismatch));
        assert_eq!(audit_control_events(&[
            ControlEvent::ConditionChecked { span: point(), selected: true }
        ]), Err(ControlAuditError::EventOutsideFunction));
    }
}
