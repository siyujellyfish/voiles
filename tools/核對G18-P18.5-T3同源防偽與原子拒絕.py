"""T3 原來源、退出事件及工作歸屬驗收。"""
from g18_matrix_gate import verify

ROWS = (
    ('UTF-8、原 Token 與 HIR', (
        'struct_stage::g18_private_recursive_str_frames_tests::g18_c_源碼或預備的巢狀資源被篡改須在零配置前拒絕',
        'struct_stage::g18_t3_async_self_token_provenance_recheck_tests::t3_async_token_偷改方法名Token但TypedHIR不變仍須拒絕',
        'struct_stage::g18_private_recursive_str_frames_tests::g18_c_竄改遞迴_int_實參來源或返回_hir_必須在真資源建立前拒絕',
    )),
    ('原 Span、AST 與借票', (
        'struct_stage::g18_t3_async_self_outcome_origin_binding_tests::t3_async_origin_未啟動工作呼叫Span變造不得簽發等待憑據',
        'struct_stage::g18_t3_async_self_token_provenance_recheck_tests::t3_async_token_偷換AST接收者但原Lexer文字不變必拒',
        'struct_stage::g18_private_scalar_call_tests::g18_r05_巢狀回滾交換內外票據歸還順序須拒絕',
    )),
    ('不同合法 Outcome 與安全點', (
        'struct_stage::g18_async_method_graph_tests::a2_另一合法工作的完成收據不可用於退休',
        'struct_stage::g18_async_method_graph_tests::a2_安全點收據與方法來源不得重播',
    )),
    ('FIFO、全批預檢與零副作用', (
        'struct_stage::g18_t3_matrix_tests::t3_較早工作事件污染整批退出必須零副作用',
        'struct_stage::g18_t3_matrix_tests::t3_合法但不同來源計畫不能冒領等待點',
    )),
)

if __name__ == "__main__":
    verify(ROWS)
