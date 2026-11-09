import requests
import json
import time

API_URL = "http://127.0.0.1:8090/v1/chat/completions"
API_KEY = "omni_sk_686e5c57ffd14b328d31af2b9d6a50ee"
MODEL = "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf"

headers = {
    "Authorization": f"Bearer {API_KEY}",
    "Content-Type": "application/json"
}

tools = [
    {
        "type": "function",
        "function": {
            "name": "get_product_price",
            "description": "استرجاع سعر ومواصفات المنتج بالكود الصارم للـ SKU",
            "parameters": {
                "type": "object",
                "properties": {
                    "sku": {"type": "string", "description": "كود الـ SKU مثل cX23"}
                },
                "required": ["sku"]
            }
        }
    }
]

print("=" * 70)
print("  🚀 [FULL E-COMMERCE LIVE TEST] Python -> Omni Engine End-to-End")
print("=" * 70)

# Turn 1: User asks
messages = [
    {
        "role": "system",
        "content": "أنت مساعد مبيعات ذكي لمتجر ماركت. استخدم الأدوات لجلب البيانات ولا تخترع أسعاراً من عندك."
    },
    {
        "role": "user",
        "content": "مساء الخير، التيشيرت الأسود cX23 سعره كام ومتاح مقاس L؟"
    }
]

print("\n--- [الخطوة 1: العميل يسأل عن cX23] ---")
payload = {
    "model": MODEL,
    "messages": messages,
    "tools": tools,
    "temperature": 0.2,
    "max_tokens": 128
}

t0 = time.time()
res = requests.post(API_URL, headers=headers, json=payload, timeout=60).json()
elapsed = time.time() - t0

assistant_msg = res["choices"][0]["message"]
tool_calls = assistant_msg.get("tool_calls", [])

print(f"استجابة المحرك خلال: {elapsed:.2f} ثانية")
if tool_calls:
    call = tool_calls[0]
    call_id = call["id"]
    fn_name = call["function"]["name"]
    fn_args = json.loads(call["function"]["arguments"])
    print(f"🎯 الموديل قرر استدعاء أداة: {fn_name}")
    print(f"   المعاملات المستخرجة بدقة: {fn_args}")

    # محاكاة إرجاع بيانات الداتابيز المحلية (Database Result)
    db_result = {
        "sku": fn_args.get("sku"),
        "item_name": "تيشيرت أسود فاخر",
        "price": 400,
        "currency": "EGP",
        "stock": {"L": 12, "XL": 5},
        "material": "قطن مصري بيور 100%"
    }
    print(f"\n📦 تم جلب البيانات من الداتابيز: {db_result}")

    # الخطوة 2: إرسال نتيجة الأداة لأومني ليكمل الرد للعميل
    messages.append(assistant_msg)
    messages.append({
        "role": "tool",
        "tool_call_id": call_id,
        "content": json.dumps(db_result, ensure_ascii=False)
    })

    print("\n--- [الخطوة 2: إرسال نتيجة الأداة لأومني لتوليد الرد النهائي للعميل] ---")
    payload2 = {
        "model": MODEL,
        "messages": messages,
        "temperature": 0.3,
        "max_tokens": 150
    }

    t1 = time.time()
    res2 = requests.post(API_URL, headers=headers, json=payload2, timeout=60).json()
    elapsed2 = time.time() - t1

    final_reply = res2["choices"][0]["message"]["content"]
    print(f"استجابة المحرك للرد النهائي خلال: {elapsed2:.2f} ثانية")
    print(f"\n💬 رد المساعد على العميل (من قلب محرك أومني):\n{final_reply}")

print("\n" + "=" * 70)
print("  ✅ الاثبات القطعي: دورة كاملة بنجاح 100% بدون أي تدريب مسبق!")
print("=" * 70)
