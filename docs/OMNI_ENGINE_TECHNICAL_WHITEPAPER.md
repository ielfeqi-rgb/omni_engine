# Omni Engine: مواصفة المعمارية التقنية والتصميم الحسابي
**وثيقة مرجعية شاملة للنظام (Technical Whitepaper & Architectural Specification)**

---

## 1. الفلسفة التأسيسية للمحرك (Foundational Philosophy)

يُعرّف **Omni Engine** بأنه محرك توليد وتنسيق برمجي حتمي فائق الأداء مبني بلغة Rust، ومرتبط بمكتبة `llama.cpp` عبر واجهات FFI منخفضة المستوى.

الهدف الأساسي للمحرك هو كسر نمط "التوليد الأعمى" (Blind Autoregressive Generation) ونقل النماذج اللغوية (خاصة النماذج الصغيرة SLMs) من دور "المبرمج المسترسل غير المنضبط" إلى دور "وحدة حسابية ذرية موجهة" (Deterministic Processing Unit) داخل آلة حالات منتهية (State Machine) مدفوعة بنظرية البيان (Graph Theory) والتحليل الساكن.

```text
+-----------------------------------------------------------------------------+
|                                 OMNI ENGINE                                 |
|                                                                             |
|  +--------------------+     +-------------------+     +------------------+  |
|  |     Rust Host      | <-> | Synchronous FFI   | <-> |  libllama (C++)  |  |
|  |  (State Machine)   |     | (Atomic Step/Halt)|     | (Inference Core) |  |
|  +--------------------+     +-------------------+     +------------------+  |
|            |                                                    |           |
|            v                                                    v           |
|  +--------------------+                               +------------------+  |
|  | AST Gatekeeper     |                               | Persistent       |  |
|  | (Tree-Sitter / Syn)|                               | KV-Cache Matrix  |  |
|  +--------------------+                               +------------------+  |
+-----------------------------------------------------------------------------+
```

---

## 2. فيزياء الانتباه وميكانيكا الـ KV-Cache

### 2.1 معادلة الانتباه والتحكم بالماسك (Attention Mask Manipulation)
يعتمد توجيه العمال الذريين على التلاعب المباشر بمصفوفة الانتباه (Attention Mask) $M \in \mathbb{R}^{N \times N}$ داخل كيرنل الحساب:

$$\text{Attention}(Q, K, V) = \text{Softmax}\left(\frac{Q K^T}{\sqrt{d_k}} + M\right) V$$

حيث:
* $Q \in \mathbb{R}^{L \times d_k}$ مصفوفة الاستعلام للتوكنات الحالية.
* $K, V \in \mathbb{R}^{S \times d_k}$ مصفوفات المفاتيح والقيم المخزنة في الـ KV-Cache.
* $M_{i, j}$ هي قيمة القناع بين التوكن $i$ والتوكن $j$:

$$M_{i, j} = \begin{cases} 0 & \text{إذا كان التوكن } j \text{ مرئياً للتوكن } i \\ -\infty & \text{إذا كان التوكن } j \text{ محجوباً} \end{cases}$$

رياضياً، عند تطبيق الـ Softmax:
$$\exp(-\infty) = 0$$

يؤدي هذا إلى تصفير وزن الانتباه الموجه للتوكن المحجوب تماماً، مما يلغي إسهامه في جمع متجهات $V$، مع بقاء بياناته محفوظة في الذاكرة دون مسح.

### 2.2 بالوعات الانتباه (Attention Sinks)
للحفاظ على الاستقرار العددي وتفادي انهيار الحسابات النقطية (Softmax Degradation)، يفرض المحرك كشف أول 4 توكنات من السياق بشكل دائم:
$$\forall i \ge 0, \quad M_{i, 0..3} = 0$$

تعمل هذه التوكنات كـ "بالوعات انتباه" تستقبل فائض أوزان الـ Softmax غير المستخدمة وتحمي استقرار طبقات المحول (Transformer Layers).

### 2.3 حفظ الذاكرة الساكنة (Zero-Compute Retention)
الـ KV-Cache هو بنية بيانات ساكنة في الرام. بقاء التوكنات فيه لا يستهلك أي قدرة معالجة ($0.0\%$ CPU Load). يتم التعديل عليه عبر مشراط جراحي:
* **التفريغ الشريحي (Slice Truncation):**
  $$\text{llama\_kv\_cache\_seq\_rm}(\text{ctx}, \text{seq\_id}, p_0, p_1)$$
  يمسح فقط التوكنات المتذبذبة في النطاق $[p_0, p_1)$ ويبقي على السياق الأصلي كما هو دون إعادة حساب.

---

## 3. التحليل الرياضي للإنتروبيا وانشطار العقد (Node Fission)

### 3.1 حساب إنتروبيا شانون اللحظية (Instantaneous Shannon Entropy)
لا يعتمد المحرك على القواعد اللغوية لتحديد تعثر النموذج، بل يراقب دالة توزيع الاحتمالات الناتجة عن طبقة الـ Softmax عند كل خطوة توليد $t$:

$$H(t) = -\sum_{i=1}^{|V|} P(w_i) \log_2 P(w_i)$$

حيث:
* $|V|$ هو الحجم الكلي لقاموس النموذج (Vocabulary Size).
* $P(w_i)$ هو الاحتمال المحسوب للرمز $w_i$.

### 3.2 كشف التردد والانشطار (Fission Trigger)
يتم حساب المتوسط المتحرك للإنتروبيا لنافذة انزلاقية ذات طول $K$:

$$\bar{H}_K(t) = \frac{1}{K} \sum_{\tau=t-K+1}^t H(\tau)$$

إذا تحقق الشرط:
$$\bar{H}_K(t) > H_{\text{threshold}}$$

يوقف محرك Rust عملية التوليد فوراً، ويحدد نقطة القطع $T_{\text{cut}}$ عند أقرب فاصل استقرار سابق:
$$T_{\text{cut}} = \arg\min_{\tau \le t} H(\tau)$$

### 3.3 ميكانيكية الانشطار (Worker Fission Protocol)
* **تجميد العامل الأول (Worker A):** تثبيت السياق من 0 إلى $T_{\text{cut}}$ وجعله للقراءة فقط (Read-Only).
* **ولادة العامل المكمل (Worker B):** ضبط إزاحة الموضع الزمني (RoPE Offset) إلى $T_{\text{cut}} + 1$.
* **حقن شريط النية الكامنة (Epistemic Intent Anchor):** الحفاظ على أهداف المهمة محقونة في أول الـ Cache ليراها العامل الجديد بوضوح، مما يمنع فقدان النية المشتركة.

---

## 4. بنية التخطيط الشجري الحتمي (Dynamic DAG Generation)

### 4.1 الاستخراج النقطي بالمقاطعة الإجبارية (Stepwise Forcible Halting)
يتم تفكيك المشروع بالكامل ديناميكياً ودون قوالب مسبقة عبر مقاطعة النموذج قسراً عند كل سطر:
```text
[Prompt: Project Entity 1:] 
   -> LLM generates: "Block" 
   -> Rust Interrupt Engine (Halt on '\n')
[Append to KV: Project Entity 2:] 
   -> LLM generates: "World" 
   -> Rust Interrupt Engine (Halt on '\n')
[Append to KV: Project Entity 3:] 
   -> LLM generates: "Player" 
   -> Rust Interrupt Engine (Halt on '\n')
```

### 4.2 بناء مصفوفة الاعتماديات (Adjacency Matrix)
يتم تحديد العلاقات بأسئلة أحادية الاتجاه تُطرح على النموذج مع المقاطعة اللحظية:
* `"To define Player, which entity is required first?"` $\to$ `World`
* `"To define World, which entity is required first?"` $\to$ `Block`

تُبنى مصفوفة التجاور $A \in \{0, 1\}^{N \times N}$ حيث $A_{i, j} = 1$ تدل على أن العقدة $j$ تعتمد على العقدة $i$:

### 4.3 الفرز الطوبولوجي للأولويات (Topological Sorting)
يستخدم المحرك خوارزمية Kahn لتحديد الترتيب الحتمي للتنفيذ:
```text
L <- قائمة فارغة للترتيب النهائي
S <- مجموعة كل العقد التي ليس لها أطراف داخلة (In-Degree = 0)

طالما أن S ليست فارغة:
    أخرج عقدة n من S وأضفها إلى L
    لكل عقدة m يوجد سهم من n إليها:
        احذف السهم e من البيان
        إذا أصبحت m بدون أطراف داخلة:
            أضف m إلى S

إذا كان البيان يحتوي على أسهم:
    أطلق تحذير وجود حلقة مغلقة (Circular Dependency Detected)
وإلا:
    الترتيب التنفيذي الحتمي = L
```

### 4.4 التفكيك الجزئي المتفرع (Fractal Drill-Down)
عند وصول التنفيذ إلى عقدة مركبة (مثل Player):
* **النماذج الصغيرة ($N_{\text{scratch}} = 0$):** استخراج المشتقات مباشرة عبر مقاطعة التوليد النقطية (Movement, Interaction).
* **النماذج الكبيرة:** منح نافذة مسودة مؤقتة $N_{\text{sub-scratch}} \le 100$ توكن للتفكير في المعمارية الداخلية، ثم حذف توكنات المسودة من الكاش عبر الـ Truncation بمجرد استخراج أسماء المكونات.

---

## 5. بوابة التحقق الساكن (AST Gatekeeping & Verification)

### 5.1 استخراج العقود البرمجية
عند اكتمال توليد أي ملف، يمرره المحرك فوراً عبر محلل شجرة البناء اللغوي المجرد (AST) باستخدام مكتبات فحص سريعة (tree-sitter, syn):
* $\text{AST}_{\text{provides}}$: المعرفات والدوال والكلاسات المصدّرة فعلياً داخل الملف.
* $\text{AST}_{\text{requires}}$: المعرفات المستوردة والمستخدمة دون تعريف محلي.

### 5.2 جبر المجموعات وفحص "الوعد الكاذب"
يجري المحرك العمليات الجبرية التالية:

```text
                          +------------------------+
                          |   Worker Outputs File  |
                          +------------------------+
                                       |
                                       v
                          +------------------------+
                          |    Run AST Analysis    |
                          +------------------------+
                                       |
               +-----------------------+-----------------------+
               |                                               |
               v                                               v
    [Delta_ghost == Empty]                           [Delta_ghost != Empty]
               |                                               |
               v                                               v
       +---------------+                            +---------------------+
       | Commit to DAG |                            | Halt Execution      |
       +---------------+                            | In-Place Correction |
                                                    | (Inject Definition) |
                                                    +---------------------+
```

* إذا كانت $\Delta_{\text{ghost\_provides}} \neq \emptyset$:
  يُحظر انتقال العقدة إلى الـ DAG. يُحقن خطأ التحليل الساكن في الكاش مباشرة:
  ```python
  # AST ERROR: Symbol 'UserModel' missing.
  class UserModel:
  ```
  يُجبر النموذج على إكمال التعريف فقط (Strict Prefill Completion).

---

## 6. بروتوكول استشارة المطور (Human-in-the-Loop Triage)

### 6.1 حساب معامل الأثر (Impact & Centrality Score)
عند حدوث تردد مستعصٍ، يحسب المحرك درجة خروج العقدة (Out-Degree) في مصفوفة التجاور $A$:

| المسار | الشرط الرياضي | الإجراء البرمجي |
|---|---|---|
| **المسار الحرج (Critical Path)** | $\text{OutDegree}(v_i) \ge 3$ | إيقاف خيط التنفيذ فوراً، تجميد الكاش، وإرسال تنبيه للمستخدم لحسم القرار. |
| **المسار المؤجل (Deferred Path)** | $\text{OutDegree}(v_i) < 3$ | حقن علامة `OMNI_DEFERRED_DECISION`، إكمال التوليد، وعرض القرار قبل الإطلاق. |

### 6.2 ترسيخ القرارات السيادية (Sovereign Rules Injection)
القرارات الصادرة من المستخدم تُسجل في جدول دائم في Rust، وتُحقن في أول التوكنات (Prefix) لجميع العمال القادمين:
```text
[SOVEREIGN_RULES]:
- Rule #1: Hostile mobs must use external LootTable JSON format.
```
يضمن هذا عدم تكرار السؤال لنفس المعضلة في أي مرحلة لاحقة من المشروع.

---

## 7. بروتوكول التزامن وإيقاف الاستهلاك (OS-Level Synchronization)

### 7.1 التجميد دون تفريغ (Zero-Power Hardware Parking)
لتفادي استهلاك المعالج أثناء انتظار المستخدم دون مسح الـ KV-Cache:
* يدير المحرك الحسابي خيطاً مخصصاً لنظام التشغيل (`std::thread`).
* عند الحاجة لقرار بشري، يُنشئ الخيط قناة اتصال فردية (`tokio::sync::oneshot`).
* يستدعي الخيط أمر الحظر: `oneshot_rx.blocking_recv()`.
داخلياً، يستدعي نظام التشغيل استدعاء النظام الخاص بـ Futex:
```c
futex(uaddr, FUTEX_WAIT, val, NULL, NULL, 0);
```

```text
Compute Thread (C++ / llama.cpp)                UI Runtime (Rust / Tokio)
--------------------------------                -------------------------
[Active Token Generation]
       |
       v (Critical Hesitation Detected)
[Create oneshot channel]
[Send TriageRequest to UI] ------------------> [Receive Event]
[Invoke: blocking_recv()]                      [Render Prompt to User]
       |                                                  |
       | (Thread parked via OS futex)                     v
       | (CPU Usage drops to 0.0%)               [Wait for User Input...]
       | (KV-Cache preserved in VRAM)                     |
       |                                                  v
       |                                         [User Submits Choice]
       | <-------------------------------------- [Send via oneshot_tx]
[Woken up by OS Scheduler]
[Inject choice into KV-Cache]
[Resume Forward Pass]
```

### 7.2 حالة العتاد أثناء التجميد
* **المعالج (CPU):** يتحول استهلاك الخيط إلى $0.0\%$؛ حيث يُنقل إلى قائمة الانتظار (WaitQueue).
* **كارت الشاشة (GPU):** تتوقف كيرنلات CUDA تماماً، ويهبط استهلاك الطاقة للحد الأدنى (Idle P-State) مع بقاء الـ Tensors محجوزة بالكامل في الـ VRAM.
* **زمن الاستجابة (Wakeup Latency):** عند إرسال المستخدم للرد، يستيقظ الخيط خلال زمن يقل عن $5\,\mu\text{s}$ ويستأنف التوليد من التوكن التالي مباشرة.

---

## 8. ملخص المواصفات البرمجية للمحرك

| المكون البرمجي | الوظيفة الرياضية والبرمجية |
| :--- | :--- |
| **Memory Retention** | إدارة الـ KV-Cache بالماسك دون تفريغ دوري |
| **Mask Bounds** | مصفوفة ثنائية ($0$ / $-\infty$) مع حماية Sink Tokens |
| **Hesitation Monitor** | حساب إنتروبيا شانون اللحظية ومراقبة تجاوز الحد |
| **Node Fission** | انشطار العقد عند التردد وتمرير الـ Intent السابق |
| **Topology Resolution** | فرز طوبولوجي لمصفوفة التجاور بدون قوالب جاهزة (Kahn's Algorithm) |
| **Stepwise Halter** | استخراج الكيانات بمقاطعة التوليد عند كل رمز (`Halt on '\n'`) |
| **Verification** | مقارنة مخرجات الـ AST مع الإعلانات بجبر المجموعات ($\Delta_{\text{ghost}}$) |
| **Sync Primitive** | تجميد الخيوط بحظر صفري للطاقة عبر Futex/Oneshot ($0.0\%$ CPU) |

---
*تم توثيق هذه المواصفة كمرجع هندسي رسمي لتطوير وتثبيت معمارية Omni Engine.*
