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
    return llama_tokenize(vocab, text, len, out_tokens, max_tokens, add_bos, false);
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

    int written = llama_token_to_piece(vocab, token, buf, buf_size, 0, false);
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
