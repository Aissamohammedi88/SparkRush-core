```python
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
from peft import LoraConfig, get_peft_model, prepare_model_for_kbit_training

class Spark1_5BAgentic:
def __init__(self, device="cuda:0", max_memory=None):
# 🚀 QUANTIZATION FP8 (4x moins de RAM que FP16)
self.quant_config = BitsAndBytesConfig(
load_in_8bit=True,
bnb_8bit_quant_type="fp8", # Meilleure précision que int8
bnb_8bit_compute_dtype=torch.float16, # BF16 pour accélération
bnb_8bit_use_double_quant=False,
)

# 🔥 CHARGEMENT DU MODELE (Spark-1.5B-Agentic)
model_name = "SparkAI/Spark-1.5B-Agentic" # Notre modèle custom
self.tokenizer = AutoTokenizer.from_pretrained(model_name)
self.tokenizer.pad_token = self.tokenizer.eos_token

self.model = AutoModelForCausalLM.from_pretrained(
model_name,
quantization_config=self.quant_config,
device_map="auto", # Auto-distribution sur multi-GPUs
attn_implementation="flash_attention_3", # 🔥 PLUS RAPIDE QUE FLASH-ATTENTION-2
torch_dtype=torch.float16,
)

# 🛠️ LOAD ADAPTÉ POUR LES AGENTS
self.model = prepare_model_for_kbit_training(self.model)
pour les la production.
def generate(self, prompt: str, max_tokens=512, temperature=0.2):
inputs = self.tokenizer(prompt, return_tensors="pt").to(self.model.device)
outputs = self.model.generate(
**inputs,
max_new_tokens=max_tokens,
do_sample=True,
temperature=temperature,
pad_token_id=self.tokenizer.eos_token_id,
)
return self.tokenizer.decode(outputs[0], skip_special_tokens=True)
```
