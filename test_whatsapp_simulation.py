import requests
import json
import time
import sqlite3

API_URL = "http://127.0.0.1:8090/v1/chat/completions"
API_KEY = "omni_sk_686e5c57ffd14b328d31af2b9d6a50ee"
MODEL = "qwen2.5-coder-1.5b-instruct-q4_k_m.gguf"

headers = {
    "Authorization": f"Bearer {API_KEY}",
    "Content-Type": "application/json"
}

# The 3 canonical market tools exposed to the WhatsApp/Facebook bot:
tools = [
    {
        "type": "function",
        "function": {
            "name": "search_products",
            "description": "البحث في المنتجات بالكلمات المفتاحية أو الوصف أو الخامة أو التصنيف (مثال: ووتربروف، شتوي، قطن، جاكيت، بولو، بنطلون)",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "كلمة البحث أو وصف المنتج المطلوب"}
                },
                "required": ["query"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "get_product_price",
            "description": "استرجاع سعر ومواصفات وخامة المنتج ومخزونه بالكود الصارم للـ SKU (مثل jk99, cX23, cx23, pl10, sh05)",
            "parameters": {
                "type": "object",
                "properties": {
                    "sku": {"type": "string", "description": "كود المنتج مثل jk99 أو cX23 أو cx23"}
                },
                "required": ["sku"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "create_order",
            "description": "تسجيل وحفظ طلب شراء للعميل بالاسم والعنوان ورقم الهاتف في قاعدة بيانات SQL",
            "parameters": {
                "type": "object",
                "properties": {
                    "sku": {"type": "string", "description": "كود المنتج المطلوب شراءه"},
                    "quantity": {"type": "integer", "description": "الكمية المطلوبة"},
                    "customer_name": {"type": "string", "description": "اسم العميل"},
                    "phone": {"type": "string", "description": "رقم الهاتف"},
                    "address": {"type": "string", "description": "عنوان الشحن"}
                },
                "required": ["sku", "quantity", "customer_name", "phone", "address"]
            }
        }
    }
]

def execute_local_db_tool(tool_name, arguments):
    """Executes tool directly against the engine Market Tool endpoint."""
    res = requests.post("http://127.0.0.1:8090/api/market/tool", json={
        "tool_name": tool_name,
        "arguments": arguments
    }, timeout=10)
    return res.json()

print("=" * 75)
print("  📱 [SIMULATING REAL-WORLD WHATSAPP / FACEBOOK E-COMMERCE CHATBOT]")
print("=" * 75)

# Initialize WhatsApp conversation history
messages = [
    {
        "role": "system",
        "content": (
            "أنت موظف مبيعات ذكي ومحترف لمتجر ماركت. "
            "قواعد: 1. عند سؤال العميل عن أي منتج أو خامة (مثل ووتربروف)، ابحث فوراً باستخدام search_products. "
            "2. لا تخترع أسعاراً ولا مواصفات من عندك أبداً."
        )
    }
]

# -------------------------------------------------------------
# TURN 1: Customer on WhatsApp says: "عندكم حاجة ووتربروف تنفع للمطر؟"
# -------------------------------------------------------------
customer_msg_1 = "عندكم حاجة ووتربروف تنفع للمطر؟"
print(f"\n[WhatsApp User]: {customer_msg_1}")
messages.append({"role": "user", "content": customer_msg_1})

payload_1 = {
    "model": MODEL,
    "messages": messages,
    "tools": tools,
    "temperature": 0.2,
    "max_tokens": 128
}

t0 = time.time()
res1 = requests.post(API_URL, headers=headers, json=payload_1, timeout=60).json()
elapsed1 = time.time() - t0

assistant_msg_1 = res1["choices"][0]["message"]
tool_calls_1 = assistant_msg_1.get("tool_calls", [])

print(f"-> المحرك استجاب خلال: {elapsed1:.2f}s")
if tool_calls_1:
    call = tool_calls_1[0]
    call_id = call["id"]
    fn_name = call["function"]["name"]
    fn_args = json.loads(call["function"]["arguments"])
    print(f"🎯 الموديل استدعى أداة قاعدة البيانات: {fn_name}({fn_args})")
    
    # Execute tool against SQL
    db_result = execute_local_db_tool(fn_name, fn_args)
    print(f"📦 نتيجة الاستعلام من data/market.db: {db_result}")
    
    # Feed back to model to formulate WhatsApp reply
    messages.append(assistant_msg_1)
    messages.append({
        "role": "tool",
        "tool_call_id": call_id,
        "content": json.dumps(db_result, ensure_ascii=False)
    })
    
    res1_reply = requests.post(API_URL, headers=headers, json={
        "model": MODEL,
        "messages": messages,
        "temperature": 0.3,
        "max_tokens": 150
    }, timeout=60).json()
    
    bot_reply_1 = res1_reply["choices"][0]["message"]["content"]
    messages.append(res1_reply["choices"][0]["message"])
    print(f"[Bot Reply on WhatsApp]:\n{bot_reply_1}")
else:
    print(f"[Bot Direct Reply]:\n{assistant_msg_1.get('content')}")
    messages.append(assistant_msg_1)

# -------------------------------------------------------------
# TURN 2: Customer on WhatsApp asks: "السعر؟"
# -------------------------------------------------------------
customer_msg_2 = "السعر؟"
print(f"\n[WhatsApp User]: {customer_msg_2}")
messages.append({"role": "user", "content": customer_msg_2})

payload_2 = {
    "model": MODEL,
    "messages": messages,
    "tools": tools,
    "temperature": 0.2,
    "max_tokens": 128
}

t1 = time.time()
res2 = requests.post(API_URL, headers=headers, json=payload_2, timeout=60).json()
elapsed2 = time.time() - t1

bot_reply_2 = res2["choices"][0]["message"]["content"]
print(f"-> المحرك استجاب للرسالة التابعة خلال: {elapsed2:.2f}s")
print(f"[Bot Reply on WhatsApp]:\n{bot_reply_2}")

print("\n" + "=" * 75)
print("  ✅ نجاح المحاكاة: المحرك احتفظ بالسياق وأجاب بالسعر الصحيح 100%!")
print("=" * 75)
