// G18／R05：普通函式與同步 self 方法共用的動態進退／借用票據對帳。
// 這只核對已實際發出的執行期事件，不產生語言層 return、例外或
// 所有權授權；原 Checker、OwnedStr ResourceId 及各執行器的細帳照舊。
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateSharedCallExitEvent {
    LoanAcquired(crate::borrow_safety::BorrowTicket),
    FunctionEntered,
    /// Copyable 局部真進／離域責任共用框架驗收，不冒充 ResourceId。
    /// 已由 Checker 宣告但尚未初始化的 var int／bool，不得冒稱
    /// OwnedStr 資源或真純值槽；只有初次賦值後才變成已初始化。
    LocalDeclaredEmpty(crate::ownership_ledger::DeclarationId),
    LocalInitialized(crate::ownership_ledger::DeclarationId),
    LocalAssigned(crate::ownership_ledger::DeclarationId),
    LocalScopeEnded(crate::ownership_ledger::DeclarationId),
    LocalScopeEndedEmpty(crate::ownership_ledger::DeclarationId),
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
    // index 0 是 main 基底詞彙作用域；每次真 FunctionEntered
    // 增加一個 frame 局部棧，離域前必須清空。
    let mut local_scopes = vec![
        Vec::<(crate::ownership_ledger::DeclarationId, bool)>::new()
    ];
    // 真票據身分與取得時所在的動態函式深度必須一起留存。
    // 同一 frame 中獨立來源的多張共享票據可以按原 Checker
    // 有效期分別歸還；但內層 frame 新取得的票據尚有效時，
    // 外層票據不准先還，否則僅比對票據張數會漏掉巢狀錯序。
    let mut tickets = Vec::<(crate::borrow_safety::BorrowTicket, usize)>::new();
    // HC14 真 BorrowTicket 是一次性身分：歸還後不能於下一個
    // 普通函式或同步 self 框架再借入同一張已作廢票據。
    let mut issued_tickets = Vec::<crate::borrow_safety::BorrowTicket>::new();
    let mut unwinding = false;
    let mut entered_any = false;
    for event in events {
        match event {
            PrivateSharedCallExitEvent::LoanAcquired(ticket) => {
                // 已 Returned 的框架不能在清理期間重新借入；這不是
                // 另一個合法的新引數求值位置。
                if unwinding || issued_tickets.contains(ticket)
                    || frames.last().is_some_and(|returned| *returned) {
                    return false;
                }
                issued_tickets.push(*ticket);
                tickets.push((*ticket, frames.len()));
            }
            PrivateSharedCallExitEvent::FunctionEntered => {
                if unwinding || frames.last().is_some_and(|returned| *returned) {
                    return false;
                }
                frames.push(false);
                local_scopes.push(Vec::new());
                entered_any = true;
            }
            PrivateSharedCallExitEvent::LocalDeclaredEmpty(id) => {
                if unwinding || frames.last().is_some_and(|returned| *returned) {
                    return false;
                }
                let Some(locals) = local_scopes.last_mut() else { return false; };
                if locals.iter().any(|(old, _)| old == id) { return false; }
                locals.push((*id, false));
            }
            PrivateSharedCallExitEvent::LocalInitialized(id) => {
                if unwinding || frames.last().is_some_and(|returned| *returned) {
                    return false;
                }
                let Some(locals) = local_scopes.last_mut() else { return false; };
                if let Some((_, initialized)) = locals.iter_mut()
                    .find(|(old, _)| old == id) {
                    if *initialized { return false; }
                    *initialized = true;
                } else {
                    locals.push((*id, true));
                }
            }
            PrivateSharedCallExitEvent::LocalAssigned(id) => {
                if unwinding || frames.last().is_some_and(|returned| *returned)
                    || local_scopes.last().is_none_or(|locals|
                        !locals.iter().any(|(local, ready)|
                            local == id && *ready)) {
                    return false;
                }
            }
            PrivateSharedCallExitEvent::LocalScopeEnded(id) => {
                let Some(locals) = local_scopes.last_mut() else { return false; };
                if locals.pop() != Some((*id, true)) { return false; }
            }
            PrivateSharedCallExitEvent::LocalScopeEndedEmpty(id) => {
                let Some(locals) = local_scopes.last_mut() else { return false; };
                if locals.pop() != Some((*id, false)) { return false; }
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
                // 成功還票必須位於真 Returned 之後、FunctionExited
                // 之前；故障路徑必須已正式 UnwindStarted。僅確認
                // 票據數量平衡不能取代這條跨執行器共通因果次序。
                if !unwinding && !frames.last().is_some_and(|returned| *returned) {
                    return false;
                }
                let Some(index) = tickets.iter()
                    .position(|(active, _)| active == ticket) else {
                    return false;
                };
                let depth = tickets[index].1;
                // 內層真 frame 在外層票據取得後新簽的 BorrowTicket
                // 必須先退場。相同深度的獨立借用不強制 LIFO，
                // 不覆寫 VB14／P15 已核准的詞彙最後使用點。
                if depth > frames.len()
                    || tickets.iter().any(|(_, active_depth)| *active_depth > depth) {
                    return false;
                }
                tickets.remove(index);
            }
            PrivateSharedCallExitEvent::FunctionExited { returned } => {
                // 此函式在目前深度取得的票據必須先退出，再能
                // FunctionExited；不能讓票據在已銷毀的 frame
                // 外「漂浮」直到下一個同深度兄弟函式才歸還。
                // 較淺外層票據仍允許在合法內層使用期內存活，
                // 符合 self 借用於 Caller 簽發、由方法歸還的情況。
                let depth = frames.len();
                if tickets.iter().any(|(_, acquired_depth)| *acquired_depth == depth) {
                    return false;
                }
                let Some(actual) = frames.pop() else { return false; };
                if actual != *returned || (unwinding && *returned)
                    || (!unwinding && !*returned) {
                    return false;
                }
                let Some(locals) = local_scopes.pop() else { return false; };
                if !locals.is_empty() { return false; }
            }
        }
    }
    frames.is_empty() && tickets.is_empty()
        && local_scopes.len() == 1 && local_scopes[0].is_empty()
        && unwinding == !succeeded
        && (!succeeded || entered_any)
}
