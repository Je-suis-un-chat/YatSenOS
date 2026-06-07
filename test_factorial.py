#!/usr/bin/env python3
"""自动化测试 YatSenOS factorial 用户程序"""
import pexpect
import sys
import time
import os

QEMU_CMD = (
    "qemu-system-x86_64 "
    "-bios yyos/assets/OVMF.fd "
    "-net none "
    "-m 96M "
    "-nographic "
    "-drive format=raw,file=fat:rw:yyos/esp"
)

def mod_fact(n, mod):
    result = 1
    for i in range(1, n + 1):
        result = (result * i) % mod
    return result

def run_test(n_value, expected_result, timeout=120):
    """运行 factorial 并验证结果"""
    print(f"\n{'='*60}")
    print(f"测试 n = {n_value}")
    print(f"预期结果 = {expected_result}")
    print(f"{'='*60}")

    child = pexpect.spawn(QEMU_CMD, encoding='utf-8', timeout=timeout, cwd='/home/gyy/YatSenOS')

    try:
        # 等待 shell 提示符出现
        idx = child.expect(['yyos> ', pexpect.TIMEOUT, pexpect.EOF], timeout=30)
        if idx != 0:
            print(f"[✗] Shell 未启动 (idx={idx})")
            print(f"Before: {child.before}")
            return False
        print("[✓] Shell 已启动")

        # 输入 factorial 命令运行程序
        child.sendline('factorial')
        
        # 等待 "Input n:" 提示
        idx = child.expect(['Input n:', pexpect.TIMEOUT, pexpect.EOF], timeout=10)
        if idx != 0:
            print(f"[✗] factorial 程序未启动 (idx={idx})")
            return False
        print("[✓] factorial 程序已启动")

        # 输入 n 的值
        child.sendline(str(n_value))
        
        # 等待结果 - 使用更灵活的匹配
        expected_str = f"The factorial of {n_value} under modulo 1000000007 is {expected_result}."
        idx = child.expect([expected_str, pexpect.TIMEOUT, pexpect.EOF], timeout=timeout)
        if idx != 0:
            print(f"[✗] 未找到预期结果 (idx={idx})")
            remaining = child.before[-1000:] if child.before else "empty"
            print(f"剩余输出: {remaining}")
            return False
        print(f"[✓] 结果正确: {expected_str}")
        
        # 等待进程退出信息
        idx = child.expect(['exited with code 0', pexpect.TIMEOUT, pexpect.EOF], timeout=10)
        if idx != 0:
            print(f"[✗] 进程退出信息不匹配")
            return False
        print("[✓] 进程正常退出 (code 0)")
        
        print(f"\n[✓✓✓] 测试 n={n_value} 通过!")
        return True

    except pexpect.TIMEOUT:
        print(f"\n[✗] 超时! n={n_value}")
        return False
    except pexpect.EOF:
        print(f"\n[✗] QEMU 意外退出! n={n_value}")
        return False
    finally:
        child.close(force=True)
        time.sleep(1)

def main():
    os.system('pkill -f "qemu-system-x86_64" 2>/dev/null')
    time.sleep(1)

    test_cases = [
        (10,   mod_fact(10, 1000000007),   30),
        (100,  mod_fact(100, 1000000007),  60),
        (999999, 128233642,                300),
    ]

    all_passed = True
    for n_val, expected, tmo in test_cases:
        if not run_test(n_val, expected, timeout=tmo):
            all_passed = False
            break

    if all_passed:
        print("\n" + "="*60)
        print("  ALL TESTS PASSED!")
        print("="*60)
    else:
        print("\n" + "="*60)
        print("  TEST FAILED!")
        print("="*60)
        sys.exit(1)

if __name__ == '__main__':
    main()