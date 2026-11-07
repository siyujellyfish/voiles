// G18／R05：普通函式與同步 self 方法共用的動態進退／借用票據對帳。
// 這只核對已實際發出的執行期事件，不產生語言層 return、例外或
// 所有權授權；原 Checker、OwnedStr ResourceId 及各執行器的細帳照舊。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateSharedCallExitEvent {
    LoanAcquired(crate::borrow_safety::BorrowTicket),
    FunctionEntered,
    Returned,
    UnwindStarted,
    LoanReturned(crate::borrow_safety::BorrowTicket),
    FunctionExited { returned: bool },
}

/// 共通的框架進出與真票據單次歸還契約：
/// * 已完成的正常框架必先 Returned 才能 Exited(true)；
/// * 錯誤退出必有唯一 UnwindStarted，後續不得新進入或假成功；
/// * 真 BorrowTicket 只能取得與歸還各一次，所有框架與票據均須清空。
/// 僅「尚未進任何方法且沒有事件的預執行錯誤」可使用空 trace；
/// 原 ScalarCall 的零預算無事件要求另於自身入口嚴格檢查。
fn private_shared_call_exit_balanced(
    events: &[PrivateSharedCallExitEvent], succeeded: bool,
) -> bool {
    if events.is_empty() {
        return !succeeded;
    }
    let mut frames = Vec::<bool>::new();
    let mut tickets = Vec::<crate::borrow_safety::BorrowTicket>::new();
    let mut unwinding = false;
    let mut entered_any = false;
    for event in events {
        match event {
            PrivateSharedCallExitEvent::LoanAcquired(ticket) => {
                if unwinding || tickets.contains(ticket) { return false; }
                tickets.push(*ticket);
            }
            PrivateSharedCallExitEvent::FunctionEntered => {
                if unwinding || frames.last().is_some_and(|returned| *returned) {
                    return false;
                }
                frames.push(false);
                entered_any = true;
            }
            PrivateSharedCallExitEvent::Returned => {
                if unwinding { return false; }
                let Some(returned) = frames.last_mut() else { return false; };
                if *returned { return false; }
                *returned = true;
            }
            PrivateSharedCallExitEvent::UnwindStarted => {
                if unwinding || succeeded { return false; }
                unwinding = true;
            }
            PrivateSharedCallExitEvent::LoanReturned(ticket) => {
                let Some(index) = tickets.iter().position(|active| active == ticket) else {
                    return false;
                };
                tickets.remove(index);
            }
            PrivateSharedCallExitEvent::FunctionExited { returned } => {
                let Some(actual) = frames.pop() else { return false; };
                if actual != *returned || (unwinding && *returned)
                    || (!unwinding && !*returned) {
                    return false;
                }
            }
        }
    }
    frames.is_empty() && tickets.is_empty()
        && unwinding == !succeeded
        && (!succeeded || entered_any)
}
