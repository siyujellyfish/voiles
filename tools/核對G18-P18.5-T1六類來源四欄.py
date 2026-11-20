"""六類來源四欄正式驗收。"""
from g18_matrix_gate import verify

ROWS = (
    ('t1_f_普通呼叫四欄', (
        'struct_stage::g18_t1_matrix_tests::t1_f_普通呼叫四欄',
    )),
    ('t1_c_選中迴圈路徑四欄', (
        'struct_stage::g18_t1_matrix_tests::t1_c_選中迴圈路徑四欄',
    )),
    ('t1_r_遞迴內層溢位與逐層資源四欄', (
        'struct_stage::g18_t1_matrix_tests::t1_r_遞迴內層溢位與逐層資源四欄',
    )),
    ('t1_b_借用形參正常與故障還票四欄', (
        'struct_stage::g18_t1_matrix_tests::t1_b_借用形參正常與故障還票四欄',
    )),
    ('t1_m_獨占方法動態退出四欄', (
        'struct_stage::g18_t1_matrix_tests::t1_m_獨占方法動態退出四欄',
    )),
    ('t1_a_直接等待方法與普通框架四欄', (
        'struct_stage::g18_t1_matrix_tests::t1_a_直接等待方法與普通框架四欄',
    )),
)

if __name__ == "__main__":
    verify(ROWS, 'struct_stage::g18_t1_matrix_tests::')
