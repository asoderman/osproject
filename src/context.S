
.intel_syntax noprefix

.global switch_context_inner
switch_context_inner:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    pushfq

    mov [rdi], rsp
    mov rsp, [rsi]

    cli

    popfq

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx

    ret

