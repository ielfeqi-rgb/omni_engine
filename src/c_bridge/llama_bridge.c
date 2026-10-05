#include <stdio.h>
#include <stdlib.h>
#include <string.h>



#include <stdbool.h>
#include <math.h>

#include "include/llama.h"

// ---------------------------------------------------------------------------
// Backend Lifecycle
// ---------------------------------------------------------------------------

void omni_llama_backend_init(void) {
    llama_backend_init();
}

void omni_llama_backend_free(void) {
    llama_backend_free();
}

// ---------------------------------------------------------------------------
// Model Management
// ---------------------------------------------------------------------------

struct llama_model * omni_llama_load_model(const char * path, int n_gpu_layers) {
    if (!path) return NULL;
    struct llama_model_params mparams = llama_model_default_params();
    mparams.n_gpu_layers = n_gpu_layers;
    return llama_model_load_from_file(path, mparams);
}

void omni_llama_free_model(struct llama_model * model) {
    if (model) {
        llama_model_free(model);
    }
}

// ---------------------------------------------------------------------------
// Context Management
// ---------------------------------------------------------------------------

struct llama_context * omni_llama_new_context(
    struct llama_model * model,
    int n_ctx,
    int n_batch,
    int n_threads
) {
    if (!model) return NULL;
    struct llama_context_params cparams = llama_context_default_params();
    cparams.n_ctx = (n_ctx > 0) ? n_ctx : 2048;
    cparams.n_batch = (n_batch > 0) ? n_batch : 512;
    cparams.n_threads = (n_threads > 0) ? n_threads : 4;
    cparams.n_threads_batch = cparams.n_threads;
    return llama_init_from_model(model, cparams);
}

void omni_llama_free_context(struct llama_context * ctx) {
    if (ctx) {
        llama_free(ctx);
    }
}

// ---------------------------------------------------------------------------
// Tokenization & Vocab
// ---------------------------------------------------------------------------

int omni_llama_tokenize(
    struct llama_model * model,
    const char * text,
    int text_len,
    int32_t * out_tokens,
    int max_tokens,
    bool add_bos
) {
    if (!model || !text || !out_tokens || max_tokens <= 0) return -1;
    const struct llama_vocab * vocab = llama_model_get_vocab(model);
    if (!vocab) return -1;

    int len = (text_len >= 0) ? text_len : (int)strlen(text);
    return llama_tokenize(vocab, text, len, out_tokens, max_tokens, add_bos, true);
}

int omni_llama_token_to_piece(
    struct llama_model * model,
    int32_t token,
    char * buf,
    int buf_size
) {
    if (!model || !buf || buf_size <= 0) return -1;
    const struct llama_vocab * vocab = llama_model_get_vocab(model);
    if (!vocab) return -1;

    int written = llama_token_to_piece(vocab, token, buf, buf_size, 0, true);
    if (written >= 0 && written < buf_size) {
        buf[written] = '\0';
    }
    return written;
}

// ---------------------------------------------------------------------------
// Decoding & Sampling
// ---------------------------------------------------------------------------

int omni_llama_eval_tokens(
    struct llama_context * ctx,
    const int32_t * tokens,
    int n_tokens,
    int seq_id,
    int start_pos
) {
    if (!ctx || !tokens || n_tokens <= 0) return -1;

    struct llama_batch batch = llama_batch_init(n_tokens, 0, 1);
    for (int i = 0; i < n_tokens; i++) {
        batch.token[i] = tokens[i];
        batch.pos[i] = (start_pos >= 0) ? (start_pos + i) : i;
        batch.n_seq_id[i] = 1;
        batch.seq_id[i][0] = seq_id;
        batch.logits[i] = (i == n_tokens - 1); // request logits for last token
    }
    batch.n_tokens = n_tokens;

    int res = llama_decode(ctx, batch);
    llama_batch_free(batch);
    return res;
}

int omni_llama_sample_greedy(struct llama_context * ctx, struct llama_model * model) {
    if (!ctx || !model) return -1;
    const struct llama_vocab * vocab = llama_model_get_vocab(model);
    if (!vocab) return -1;

    float * logits = llama_get_logits(ctx);
    if (!logits) return -1;

    int n_vocab = llama_vocab_n_tokens(vocab);
    int best_id = 0;
    float best_logit = -INFINITY;

    for (int i = 0; i < n_vocab; i++) {
        if (logits[i] > best_logit) {
            best_logit = logits[i];
            best_id = i;
        }
    }
    return best_id;
}

int omni_llama_n_vocab(struct llama_model * model) {
    if (!model) return -1;
    const struct llama_vocab * vocab = llama_model_get_vocab(model);
    if (!vocab) return -1;
    return (int)llama_vocab_n_tokens(vocab);
}

int omni_llama_get_logits(struct llama_context * ctx, struct llama_model * model, float * out_logits, int max_vocab) {
    if (!ctx || !model || !out_logits || max_vocab <= 0) return -1;
    const struct llama_vocab * vocab = llama_model_get_vocab(model);
    if (!vocab) return -1;
    float * logits = llama_get_logits_ith(ctx, -1);
    if (!logits) {
        logits = llama_get_logits(ctx);
    }
    if (!logits) return -1;
    int n_vocab = (int)llama_vocab_n_tokens(vocab);
    int to_copy = (max_vocab < n_vocab) ? max_vocab : n_vocab;
    memcpy(out_logits, logits, to_copy * sizeof(float));
    return to_copy;
}

// ---------------------------------------------------------------------------
// REAL KV-CACHE MANIPULATION API
// ---------------------------------------------------------------------------

int omni_llama_kv_cache_used_cells(struct llama_context * ctx) {
    if (!ctx) return -1;
    return (int)llama_get_kv_cache_used_cells(ctx);
}

int omni_llama_kv_cache_token_count(struct llama_context * ctx) {
    if (!ctx) return -1;
    return (int)llama_get_kv_cache_token_count(ctx);
}

void omni_llama_kv_cache_clear(struct llama_context * ctx) {
    if (ctx) {
        llama_kv_cache_clear(ctx);
    }
}

bool omni_llama_kv_cache_seq_rm(
    struct llama_context * ctx,
    int seq_id,
    int p0,
    int p1
) {
    if (!ctx) return false;
    return llama_kv_cache_seq_rm(ctx, (llama_seq_id)seq_id, (llama_pos)p0, (llama_pos)p1);
}

void omni_llama_kv_cache_seq_cp(
    struct llama_context * ctx,
    int seq_src,
    int seq_dst,
    int p0,
    int p1
) {
    if (ctx) {
        llama_kv_cache_seq_cp(ctx, (llama_seq_id)seq_src, (llama_seq_id)seq_dst, (llama_pos)p0, (llama_pos)p1);
    }
}

void omni_llama_kv_cache_seq_shift(
    struct llama_context * ctx,
    int seq_id,
    int p0,
    int p1,
    int delta
) {
    if (ctx) {
        llama_kv_cache_seq_add(ctx, (llama_seq_id)seq_id, (llama_pos)p0, (llama_pos)p1, (llama_pos)delta);
    }
}

int omni_llama_kv_cache_seq_pos_max(struct llama_context * ctx, int seq_id) {
    if (!ctx) return -1;
    return (int)llama_kv_cache_seq_pos_max(ctx, (llama_seq_id)seq_id);
}

// ---------------------------------------------------------------------------
// Advanced Sampler Chain API
// ---------------------------------------------------------------------------

struct omni_llama_sampler {
    struct llama_sampler * chain;
};

struct omni_llama_sampler * omni_llama_sampler_init_chain(void) {
    struct llama_sampler_chain_params sparams = llama_sampler_chain_default_params();
    sparams.no_perf = true;
    struct llama_sampler * chain = llama_sampler_chain_init(sparams);
    if (!chain) {
        return NULL;
    }

    struct omni_llama_sampler * omni_smpl = (struct omni_llama_sampler *)malloc(sizeof(struct omni_llama_sampler));
    if (!omni_smpl) {
        llama_sampler_free(chain);
        return NULL;
    }
    omni_smpl->chain = chain;
    return omni_smpl;
}

void omni_llama_sampler_add_penalties(
    struct omni_llama_sampler * chain,
    int32_t n_vocab,
    int32_t penalty_last_n,
    float penalty_repeat,
    float penalty_freq,
    float penalty_present
) {
    (void)n_vocab;
    if (!chain || !chain->chain) {
        return;
    }

    // Skip if all penalties are inactive
    if (penalty_last_n == 0 || (penalty_repeat == 1.0f && penalty_freq == 0.0f && penalty_present == 0.0f)) {
        return;
    }

    int32_t last_n = penalty_last_n;
    if (last_n <= 0 && last_n != -1) {
        last_n = 64;
    }

    struct llama_sampler * smpl = llama_sampler_init_penalties(
        last_n,
        penalty_repeat,
        penalty_freq,
        penalty_present
    );
    if (!smpl) {
        return;
    }

    llama_sampler_chain_add(chain->chain, smpl);
}

void omni_llama_sampler_add_top_k(struct omni_llama_sampler * chain, int32_t k) {
    if (!chain || !chain->chain) {
        return;
    }
    if (k <= 0) {
        return;
    }

    struct llama_sampler * smpl = llama_sampler_init_top_k(k);
    if (!smpl) {
        return;
    }

    llama_sampler_chain_add(chain->chain, smpl);
}

void omni_llama_sampler_add_top_p(struct omni_llama_sampler * chain, float p, size_t min_keep) {
    if (!chain || !chain->chain) {
        return;
    }
    if (p <= 0.0f || p >= 1.0f) {
        return;
    }

    size_t keep = (min_keep > 0) ? min_keep : 1;
    struct llama_sampler * smpl = llama_sampler_init_top_p(p, keep);
    if (!smpl) {
        return;
    }

    llama_sampler_chain_add(chain->chain, smpl);
}

void omni_llama_sampler_add_min_p(struct omni_llama_sampler * chain, float p, size_t min_keep) {
    if (!chain || !chain->chain) {
        return;
    }
    if (p <= 0.0f || p >= 1.0f) {
        return;
    }

    size_t keep = (min_keep > 0) ? min_keep : 1;
    struct llama_sampler * smpl = llama_sampler_init_min_p(p, keep);
    if (!smpl) {
        return;
    }

    llama_sampler_chain_add(chain->chain, smpl);
}

void omni_llama_sampler_add_temp(struct omni_llama_sampler * chain, float temp) {
    if (!chain || !chain->chain) {
        return;
    }
    if (temp < 0.0f) {
        temp = 0.0f;
    }

    struct llama_sampler * smpl = llama_sampler_init_temp(temp);
    if (!smpl) {
        return;
    }

    llama_sampler_chain_add(chain->chain, smpl);
}

void omni_llama_sampler_add_dist(struct omni_llama_sampler * chain, uint32_t seed) {
    if (!chain || !chain->chain) {
        return;
    }

    uint32_t s = (seed != 0) ? seed : LLAMA_DEFAULT_SEED;
    struct llama_sampler * smpl = llama_sampler_init_dist(s);
    if (!smpl) {
        return;
    }

    llama_sampler_chain_add(chain->chain, smpl);
}

void omni_llama_sampler_add_greedy(struct omni_llama_sampler * chain) {
    if (!chain || !chain->chain) {
        return;
    }

    struct llama_sampler * smpl = llama_sampler_init_greedy();
    if (!smpl) {
        return;
    }

    llama_sampler_chain_add(chain->chain, smpl);
}

int32_t omni_llama_sampler_sample(
    struct omni_llama_sampler * chain,
    struct llama_context * ctx,
    int32_t idx
) {
    if (!chain || !chain->chain || !ctx) {
        return -1;
    }

    // Defensive check: verify that logits are actually available for this context.
    // If llama_decode hasn't been called or idx is invalid, llama_get_logits_ith will return NULL,
    // which would cause an unhandled SIGSEGV inside libllama's llama_sampler_sample.
    float * logits = llama_get_logits_ith(ctx, idx);
    if (!logits) {
        logits = llama_get_logits(ctx);
    }
    if (!logits) {
        return -1;
    }

    llama_token token = llama_sampler_sample(chain->chain, ctx, idx);
    return (int32_t)token;
}

void omni_llama_sampler_accept(struct omni_llama_sampler * chain, int32_t token) {
    if (!chain || !chain->chain) {
        return;
    }
    if (token < 0) {
        return;
    }
    llama_sampler_accept(chain->chain, (llama_token)token);
}

void omni_llama_sampler_reset(struct omni_llama_sampler * chain) {
    if (!chain || !chain->chain) {
        return;
    }
    llama_sampler_reset(chain->chain);
}

void omni_llama_sampler_free(struct omni_llama_sampler * chain) {
    if (!chain) {
        return;
    }
    if (chain->chain) {
        llama_sampler_free(chain->chain);
        chain->chain = NULL;
    }
    free(chain);
}

struct omni_llama_sampler * omni_llama_sampler_create(
    float temp,
    float top_p,
    int32_t top_k,
    float penalty_repeat,
    float penalty_freq,
    float penalty_present,
    int32_t penalty_last_n,
    uint32_t seed
) {
    struct omni_llama_sampler * chain = omni_llama_sampler_init_chain();
    if (!chain) {
        return NULL;
    }

    if (penalty_last_n > 0 || penalty_repeat != 1.0f || penalty_freq != 0.0f || penalty_present != 0.0f) {
        omni_llama_sampler_add_penalties(chain, 0, penalty_last_n, penalty_repeat, penalty_freq, penalty_present);
    }

    if (top_k > 0) {
        omni_llama_sampler_add_top_k(chain, top_k);
    }

    if (top_p > 0.0f && top_p < 1.0f) {
        omni_llama_sampler_add_top_p(chain, top_p, 1);
    }

    if (temp <= 0.0f) {
        omni_llama_sampler_add_greedy(chain);
    } else {
        omni_llama_sampler_add_temp(chain, temp);
        omni_llama_sampler_add_dist(chain, seed);
    }

    return chain;
}



// ---------------------------------------------------------------------------
// LOGGING API
// ---------------------------------------------------------------------------
static void dummy_log_callback(enum ggml_log_level level, const char * text, void * user_data) {
    (void)level;
    (void)text;
    (void)user_data;
}

void omni_llama_disable_logs(void) {
    llama_log_set(dummy_log_callback, NULL);
}
