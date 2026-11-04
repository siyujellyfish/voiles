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
