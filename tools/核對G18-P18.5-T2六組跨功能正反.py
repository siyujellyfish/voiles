"""六組跨功能正反正式驗收。"""
from g18_matrix_gate import verify

ROWS = (
    ('函式與控制流程', (
        'struct_stage::g18_m2_m3_obligation_tests::m3_求參與方法回傳共用迴圈中的真框架',
        'struct_stage::g18_m2_m3_obligation_tests::m3_中途故障逆序離域及子呼叫來源重驗',
    )),
    ('控制流程與借用', (
        'struct_stage::g18_r06_cross_runtime_tests::g18_r06_for_重複同一外層_str_真借用逐輪歸還',
        'struct_stage::g18_r06_cross_runtime_tests::g18_r06_借用後第二_int_實參溢位_真票據歸還先於_resourceid_釋放',
    )),
    ('借用與移動', (
        'struct_stage::g18_r06_cross_runtime_tests::g18_r06_同一真函式堆疊先_move_後借另一_source_資源不混淆',
        'struct_stage::g18_private_scalar_call_tests::g18_r02_main分支中已move來源再次借用必由原checker先拒絕',
    )),
    ('遞迴與失敗', (
        'struct_stage::g18_private_scalar_call_tests::g18_c_遞迴中相同靜態局部宣告每個動態框架均獨立初始化及離域',
        'struct_stage::g18_private_recursive_str_frames_tests::g18_c_混合_int_底層算術溢位須失敗並先歸還借用票據再逐層釋放',
    )),
    ('方法與票據', (
        'struct_stage::g18_m2_m3_obligation_tests::m2_交替持有者的寫入分支與參數資源',
        'struct_stage::g18_b4_obligation_tests::b4_06_方法與參數還票次序交換須拒絕',
    )),
    ('非同步與取消', (
        'struct_stage::g18_async_method_graph_tests::a2_各安全點取消時真移交與逆序離域',
        'struct_stage::g18_async_method_graph_tests::a2_錯換方法等待來源須在整批取消前拒絕',
    )),
)

if __name__ == "__main__":
    verify(ROWS)
