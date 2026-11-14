# Maintained model and provider policy — 23 October 2026

Native commands, phrasebook, accepted source/knowledge material and the offline composer run first. Desktop local generation is next when configured; online AI is optional fallback for unmatched turns. No model is compulsory, no credit top-up is made, and no download token silently authorizes funded inference.

Five offered GGUF variants under3GB span Qwen, SmolLM2 and Phi. Official pinned revisions, file sizes and SHA256 values were rechecked23October2026. Repository download popularity is not conversational quality. Download size is not working RAM. Existing known local services/caches are separately discoverable on desktop.

Permitted catalogue requests refresh public repository availability, gated/licence metadata and popularity in parallel, with8-second request timeouts and six-hour session caching. Public metadata is cached on disk for offline use. Failures retain bundled compatible choices. Cached metadata cannot replace pinned URLs/hashes or add untested architectures. New compatible architectures require tested app updates, not arbitrary exhaustive downloading. Android native managed generative GGUF runtime is not enabled; built-in offline command/retrieval/composer remains independent of it.

| Provider | Current represented access | Official policy |
|---|---|---|
| OpenRouter | Live zero-price text-model filter; rate limits | https://openrouter.ai/pricing |
| Groq | Developer free tier; model/account limits | https://console.groq.com/docs/rate-limits |
| Google Gemini | Eligible model free tier, restrictions apply | https://ai.google.dev/gemini-api/docs/pricing |
| Mistral | Free evaluation mode with limits | https://help.mistral.ai/en/articles/698531-why-am-i-hitting-api-rate-limits-and-how-do-i-increase-them |
| NVIDIA NIM | Developer prototyping, not unlimited production | https://docs.api.nvidia.com/nim/docs/product |
| Cerebras | Optional promotional credit; payment method required; not recurring free tier | https://www.cerebras.ai/pricing |
| Hugging Face | No free-user inference credits; funded/subscription use | https://huggingface.co/docs/inference-providers/pricing |

GitHub Models is excluded: https://docs.github.com/en/github-models states retirement on30July2026. Seven integrated providers do not mean seven perpetual free services; do not invent ten free choices to fill a count.

Settings provide multiple candidates and authenticated account-catalog refresh. Model visibility is not proof of a free price. Quota/service errors try other configured eligible providers with cooldowns. Funded HF/Cerebras are excluded unless explicitly selected. Other providers may also charge if the user enables a paid plan; provider spending caps apply. No user-owned authenticated inference was performed in this audit.

Maintain dated notes/account/plan links in `online_registry.ts` together with native endpoints/defaults. Model-list refresh cannot automatically rewrite contractual terms; policy requires periodic official review and tested updates. HF public model downloads and HF inference billing are different services.
