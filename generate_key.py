#!/usr/bin/env python3
"""
Hermes AI 激活码生成工具
用法: python generate_key.py <机器码前8位> <月数>
示例: python generate_key.py a1b2c3d4 1
"""
import hashlib
import sys
from datetime import date, timedelta

SALT = b"HermesAI_v1_2025"

def checksum_from_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()[:8]

def generate_activation_code(machine_code: str, months: int) -> str:
    expiry = date.today() + timedelta(days=months * 30)
    expiry_str = expiry.strftime("%Y-%m-%d")
    short_id = machine_code[:8]
    date_part = expiry_str.replace("-", "")
    data = machine_code.encode() + expiry_str.encode() + SALT
    checksum = checksum_from_bytes(data)
    return f"{short_id}-{date_part}{checksum}"

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("用法: python generate_key.py <机器码前8位> <月数>")
        print("示例: python generate_key.py a1b2c3d4 1")
        sys.exit(1)
    machine_code = sys.argv[1]
    months = int(sys.argv[2])
    if len(machine_code) < 8:
        print("错误: 机器码至少需要8位")
        sys.exit(1)
    code = generate_activation_code(machine_code, months)
    expiry_date = date.today() + timedelta(days=months * 30)
    print(f"激活码: {code}")
    print(f"到期: {expiry_date}")
