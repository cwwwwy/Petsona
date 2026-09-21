#ifndef PETSONA_H
#define PETSONA_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define PETSONA_ABI_VERSION 3u

typedef struct PetsonaEngine PetsonaEngine;

typedef struct {
    const uint8_t *ptr;
    size_t len;
} PetsonaStringView;

typedef struct {
    uint32_t abi_version;
    PetsonaStringView home;
} PetsonaEngineOptions;

typedef struct {
    uint32_t abi_version;
    uint64_t revision;
    uint8_t ready;
    uint8_t faulted;
    uint8_t has_pet;
    uint8_t pet_visible;
    uint8_t click_through;
    uint8_t auto_walk;
    uint8_t gravity_enabled;
    uint8_t always_on_top;
    uint8_t conversation_inflight;
    float scale;
    uint32_t sprite_index;
    uint32_t atlas_width;
    uint32_t atlas_height;
    uint32_t cell_width;
    uint32_t cell_height;
    uint32_t next_frame_ms;
    uint32_t conversation_history_len;
    uint16_t state_server_port;
    uint16_t reserved_tail;
} PetsonaSnapshot;

typedef enum {
    PETSONA_TEXT_STATE = 0,
    PETSONA_TEXT_PET_ID = 1,
    PETSONA_TEXT_PET_NAME = 2,
    PETSONA_TEXT_BUBBLE = 3,
    PETSONA_TEXT_ATLAS_PATH = 4,
    PETSONA_TEXT_ERROR = 5,
    PETSONA_TEXT_PETS = 6,
    PETSONA_TEXT_PERSONA_ID = 7,
    PETSONA_TEXT_PERSONA_NAME = 8,
    PETSONA_TEXT_STATUS = 9,
    PETSONA_TEXT_POSITION = 10,
    PETSONA_TEXT_CODEX_PETS = 11,
    PETSONA_TEXT_PERSONA = 12,
    PETSONA_TEXT_PERSONAS = 13,
    PETSONA_TEXT_DEEPSEEK_CONFIG = 14,
    PETSONA_TEXT_MEMORY = 15,
    PETSONA_TEXT_IMPORT_CONFLICT = 16
} PetsonaTextField;

typedef enum {
    PETSONA_COMMAND_SET_VISIBILITY = 1,
    PETSONA_COMMAND_SET_CLICK_THROUGH = 2,
    PETSONA_COMMAND_SET_SCALE = 3,
    PETSONA_COMMAND_SET_STATE = 4,
    PETSONA_COMMAND_SHOW_BUBBLE = 5,
    PETSONA_COMMAND_CLEAR_BUBBLE = 6,
    PETSONA_COMMAND_SET_POSITION = 7,
    PETSONA_COMMAND_SET_AUTO_WALK = 8,
    PETSONA_COMMAND_SET_GRAVITY = 9,
    PETSONA_COMMAND_SET_ALWAYS_ON_TOP = 10,
    PETSONA_COMMAND_REFRESH_PETS = 11,
    PETSONA_COMMAND_IMPORT_PET = 12,
    PETSONA_COMMAND_EXPORT_PET = 13,
    PETSONA_COMMAND_SELECT_PET = 14,
    PETSONA_COMMAND_DELETE_PET = 15,
    PETSONA_COMMAND_UPDATE_PERSONA = 16,
    PETSONA_COMMAND_SAVE_PERSONA = 17,
    PETSONA_COMMAND_SAVE_DEEPSEEK_KEY = 18,
    PETSONA_COMMAND_SEND_CONVERSATION = 19,
    PETSONA_COMMAND_SET_GAZE_TARGET = 20,
    PETSONA_COMMAND_CLEAR_GAZE = 21,
    PETSONA_COMMAND_SCAN_CODEX_PETS = 22,
    PETSONA_COMMAND_UPDATE_DEEPSEEK_CONFIG = 23,
    PETSONA_COMMAND_UPDATE_MEMORY_CONFIG = 24,
    PETSONA_COMMAND_REMEMBER_FACT = 25,
    PETSONA_COMMAND_FORGET_FACT = 26,
    PETSONA_COMMAND_CLEAR_MEMORY = 27,
    PETSONA_COMMAND_REFRESH_PERSONAS = 28,
    PETSONA_COMMAND_CREATE_PERSONA = 29,
    PETSONA_COMMAND_DUPLICATE_PERSONA = 30,
    PETSONA_COMMAND_SELECT_PERSONA = 31,
    PETSONA_COMMAND_DELETE_PERSONA = 32,
    PETSONA_COMMAND_IMPORT_PERSONA = 33,
    PETSONA_COMMAND_EXPORT_PERSONA = 34,
    PETSONA_COMMAND_CLEAR_IMPORT_CONFLICT = 35,
    PETSONA_COMMAND_UPDATE_GREETING_CONFIG = 36
} PetsonaCommandKind;

typedef struct {
    uint32_t kind;
    uint32_t reserved;
    double value;
    uint64_t ttl_ms;
    PetsonaStringView text;
} PetsonaCommand;

typedef enum {
    PETSONA_OK = 0,
    PETSONA_INVALID_ARGUMENT = 1,
    PETSONA_INVALID_HANDLE = 2,
    PETSONA_ALREADY_RUNNING = 3,
    PETSONA_INITIALIZATION_FAILED = 4,
    PETSONA_RUNTIME_FAILED = 5,
    PETSONA_PANIC = 6,
    PETSONA_STOPPED = 7
} PetsonaStatus;

PetsonaStatus petsona_engine_create(const PetsonaEngineOptions *options,
                                    PetsonaEngine **out_engine);
void petsona_engine_destroy(PetsonaEngine *engine);
PetsonaStatus petsona_engine_tick(PetsonaEngine *engine);
PetsonaStatus petsona_engine_snapshot(PetsonaEngine *engine,
                                      PetsonaSnapshot *destination);
size_t petsona_engine_copy_text(PetsonaEngine *engine,
                                uint32_t field,
                                uint8_t *destination,
                                size_t capacity);
PetsonaStatus petsona_engine_command(PetsonaEngine *engine,
                                      const PetsonaCommand *command);
size_t petsona_last_error_copy(uint8_t *destination, size_t capacity);

#ifdef __cplusplus
}
#endif

#endif
