#!/usr/bin/env bash

source "$(dirname "${BASH_SOURCE[0]}")/../../scripts/caching.sh"
qs_ensure_cache "music"

# Vortex: el estado va en ~/.local/state (antes en /run/user, que se borra al reiniciar y se perdía el
# ecualizador); se trae el de la carpeta vieja si existe.
STATE_FILE="$QS_STATE_MUSIC/eq_state.json"
if [ ! -f "$STATE_FILE" ] && [ -f "$QS_RUN_MUSIC/eq_state.json" ]; then
    mv -f "$QS_RUN_MUSIC/eq_state.json" "$STATE_FILE"
fi
PRESET_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/easyeffects/output"
PRESET_NAME="live_eq"
PRESET_FILE="$PRESET_DIR/${PRESET_NAME}.json"
TEST_FILE="$QS_RUN_MUSIC/channel_test.wav"
TEST_PID_FILE="$QS_RUN_MUSIC/channel_test.pid"

mkdir -p "$PRESET_DIR"

# Vortex 1.10 (idea de THYRVAREN): las bandas son una lista «bandas» de {f: Hz, g: dB} que se puede
# editar (frecuencia de cada una, agregar o quitar, de 2 a 16), en vez de 10 fijas b1…b10. Los presets
# vuelven a las 10 de siempre; un estado viejo se pasa con sus mismas ganancias.
FRECUENCIAS='[31,63,125,250,500,1000,2000,4000,8000,16000]'
MIN_BANDAS=2
MAX_BANDAS=16

if [ ! -f "$STATE_FILE" ]; then
    jq -nc --argjson F "$FRECUENCIAS" '{"bandas": [$F[] | {f: ., g: 0}], "preset": "Flat", "pending": false, "balance": 0, "mono": false, "swap": false, "dynamics": "off"}' > "$STATE_FILE"
elif ! jq -e 'has("bandas") and has("balance") and has("mono") and has("swap") and has("dynamics")' "$STATE_FILE" >/dev/null 2>&1; then
    updated=$(jq -c --argjson F "$FRECUENCIAS" '
        .balance //= 0 | .mono //= false | .swap //= false | .dynamics //= "off"
        | .bandas //= [range(10) as $i | {f: $F[$i], g: ((.["b\($i + 1)"] // 0) | tonumber? // 0)}]
        | del(.b1, .b2, .b3, .b4, .b5, .b6, .b7, .b8, .b9, .b10)' "$STATE_FILE") &&
        echo "$updated" > "$STATE_FILE"
fi

ensure_easyeffects() {
    if pgrep -x easyeffects >/dev/null 2>&1; then
        return 0
    fi

    if command -v systemctl >/dev/null 2>&1 &&
       systemctl --user cat ee-eq.service >/dev/null 2>&1; then
        systemctl --user start ee-eq.service >/dev/null 2>&1
        sleep 0.5
        return 0
    fi

    if command -v rc-service >/dev/null 2>&1 &&
       rc-service easyeffects status >/dev/null 2>&1; then
        rc-service easyeffects start >/dev/null 2>&1
        sleep 0.5
        return 0
    fi

    if command -v sv >/dev/null 2>&1 &&
       [ -d /var/service/easyeffects ]; then
        sv up /var/service/easyeffects >/dev/null 2>&1
        sleep 0.5
        return 0
    fi

    if command -v s6-svc >/dev/null 2>&1 &&
       [ -d /run/service/easyeffects ]; then
        s6-svc -u /run/service/easyeffects >/dev/null 2>&1
        sleep 0.5
        return 0
    fi

    easyeffects -w -l "$PRESET_NAME" >/dev/null 2>&1 &
    sleep 0.5
}

apply_eq() {
    vals=$(cat "$STATE_FILE")

    python3 -c "
import sys, json, math

try:
    data = json.loads(sys.argv[1])

    # Vortex 1.10: tantas bandas como haya en «bandas», cada una con su frecuencia. El ancho (Q) sale de
    # la distancia en octavas a sus vecinas: con más bandas, cada una es más angosta.
    bandas = sorted(
        ({'f': max(20.0, min(20000.0, float(b['f']))), 'g': max(-12.0, min(12.0, float(b['g'])))}
         for b in data['bandas']),
        key=lambda b: b['f'])[:32]
    if not bandas:
        bandas = [{'f': 1000.0, 'g': 0.0}]

    mono = bool(data.get('mono', False))
    swap = bool(data.get('swap', False))
    balance = max(-1.0, min(1.0, float(data.get('balance', 0.0))))
    plugin_balance = 0.0 if mono else (-balance if swap else balance) * 100.0
    dynamics = str(data.get('dynamics', 'off'))
    if dynamics not in ('off', 'balanced', 'night'):
        dynamics = 'off'

    bands = {}
    for i, b in enumerate(bandas):
        lados = []
        if i > 0:
            lados.append(math.log2(b['f'] / bandas[i - 1]['f']))
        if i < len(bandas) - 1:
            lados.append(math.log2(bandas[i + 1]['f'] / b['f']))
        octavas = max(0.1, min(4.0, sum(lados) / len(lados) if lados else 1.0))
        q = math.sqrt(2 ** octavas) / (2 ** octavas - 1)
        bands[f'band{i}'] = {
            'frequency': round(b['f'], 1),
            'gain': b['g'],
            'mode': 'Bell',
            'mute': False,
            'q': round(max(0.3, min(10.0, q)), 3),
            'solo': False,
            'width': 1.0,
            'slope': 'x1'
        }

    output = {
        'blocklist': [],
        'plugins_order': ['equalizer'],
        'equalizer': {
                'balance': plugin_balance,
                'bypass': False,
                'input-gain': 0.0,
                'output-gain': 0.0,
                'left': bands,
                'right': bands,
                'mode': 'IIR',
                'num-bands': len(bandas),
                'split-channels': False
        }
    }

    if mono or swap:
        output['plugins_order'].append('stereo_tools')
        output['stereo_tools'] = {
            'balance-in': 0.0,
            'balance-out': 0.0,
            'bypass': False,
            'delay': 0.0,
            'dry': -100.0,
            'input-gain': 0.0,
            'middle-level': 0.0,
            'middle-panorama': 0.0,
            'mode': ('LR > L+R (Mono Sum L+R)' if mono
                     else 'LR > RL (Stereo Flip Channels)'),
            'mutel': False,
            'muter': False,
            'output-gain': -6.0 if mono else 0.0,
            'phasel': False,
            'phaser': False,
            'sc-level': 1.0,
            'side-balance': 0.0,
            'side-level': 0.0,
            'softclip': False,
            'stereo-base': 0.0,
            'stereo-phase': 0.0,
            'wet': 0.0
        }

    if dynamics != 'off':
        night = dynamics == 'night'
        output['plugins_order'].extend(['compressor', 'limiter'])
        output['compressor'] = {
            'attack': 6.0 if night else 14.0,
            'boost-amount': 6.0,
            'boost-threshold': -72.0,
            'bypass': False,
            'dry': -100.0,
            'hpf-frequency': 10.0,
            'hpf-mode': 'off',
            'input-gain': 0.0,
            'knee': -14.0 if night else -10.0,
            'lpf-frequency': 20000.0,
            'lpf-mode': 'off',
            'makeup': 0.0,
            'mode': 'Downward',
            'output-gain': 0.0,
            'ratio': 6.0 if night else 2.8,
            'release': 320.0 if night else 200.0,
            'release-threshold': -100.0,
            'sidechain': {'lookahead': 0.0, 'mode': 'RMS', 'preamp': 0.0, 'reactivity': 10.0, 'source': 'Middle', 'stereo-split-source': 'Left/Right', 'type': 'Feed-forward'},
            'stereo-split': False,
            'input-to-sidechain': 0.0,
            'input-to-link': 0.0,
            'sidechain-to-input': 0.0,
            'sidechain-to-link': 0.0,
            'link-to-input': 0.0,
            'link-to-sidechain': 0.0,
            'threshold': -36.0 if night else -26.0,
            'wet': 0.0
        }
        output['limiter'] = {
            'alr': False,
            'alr-attack': 5.0,
            'alr-knee': 0.0,
            'alr-knee-smooth': 0.0,
            'alr-release': 50.0,
            'attack': 5.0,
            'bypass': False,
            'dithering': 'None',
            'external-sidechain': False,
            'gain-boost': False,
            'input-gain': 0.0,
            'lookahead': 5.0,
            'mode': 'Herm Thin',
            'output-gain': 0.0,
            'oversampling': 'None',
            'release': 50.0,
            'sidechain-preamp': 0.0,
            'sidechain-type': 'Feed-forward',
            'stereo-link': 100.0,
            'input-to-sidechain': 0.0,
            'input-to-link': 0.0,
            'sidechain-to-input': 0.0,
            'sidechain-to-link': 0.0,
            'link-to-input': 0.0,
            'link-to-sidechain': 0.0,
            'threshold': -1.0
        }

    preset = {'output': output}

    print(json.dumps(preset, indent=4))

except Exception:
    sys.exit(1)
" "$vals" > "$PRESET_FILE.tmp.$$" &&
    mv -f "$PRESET_FILE.tmp.$$" "$PRESET_FILE" ||
    rm -f "$PRESET_FILE.tmp.$$"

    ensure_easyeffects
    easyeffects -l "$PRESET_NAME" >/dev/null 2>&1 &
}

# Un preset vuelve a las 10 bandas de siempre: save_preset NOMBRE g1 … g10
save_preset() {
    local nombre=$1
    shift
    updated=$(jq -c --argjson F "$FRECUENCIAS" --arg p "$nombre" --args \
        '.bandas = [range(10) as $i | {f: $F[$i], g: ($ARGS.positional[$i] | tonumber)}]
         | .preset = $p | .pending = false' "$@" <"$STATE_FILE") || return 1
    echo "$updated" > "$STATE_FILE"
}

stop_test() {
    if [ -f "$TEST_PID_FILE" ]; then
        pid=$(cat "$TEST_PID_FILE" 2>/dev/null || true)
        if [[ "$pid" =~ ^[0-9]+$ ]]; then
            kill "$pid" >/dev/null 2>&1 || true
        fi
        rm -f "$TEST_PID_FILE"
    fi
}

play_test() {
    stop_test

    if [ ! -s "$TEST_FILE" ]; then
        python3 - "$TEST_FILE" <<'PY'
import math, struct, sys, wave

path = sys.argv[1]
rate = 48000
segments = [(0.75, 660, 0), (0.18, 0, 0), (0.75, 0, 880),
            (0.18, 0, 0), (1.0, 770, 770)]
with wave.open(path, 'wb') as out:
    out.setnchannels(2)
    out.setsampwidth(2)
    out.setframerate(rate)
    for duration, left_hz, right_hz in segments:
        for index in range(int(duration * rate)):
            fade = min(1.0, index / (rate * 0.015),
                       (duration * rate - index) / (rate * 0.015))
            def sample(freq):
                return int(9000 * fade * math.sin(2 * math.pi * freq * index / rate)) if freq else 0
            out.writeframesraw(struct.pack('<hh', sample(left_hz), sample(right_hz)))
PY
    fi

    ensure_easyeffects
    target=()
    if command -v pw-dump >/dev/null 2>&1 &&
       pw-dump 2>/dev/null | jq -e 'any(.[]; .info.props["node.name"] == "easyeffects_sink")' >/dev/null 2>&1; then
        target=(--target=easyeffects_sink)
    fi

    (
        player_pid=""
        cleanup_test() {
            [ -n "$player_pid" ] && kill "$player_pid" >/dev/null 2>&1 || true
            if [ "$(cat "$TEST_PID_FILE" 2>/dev/null)" = "$BASHPID" ]; then
                rm -f "$TEST_PID_FILE"
            fi
        }
        trap cleanup_test EXIT TERM INT
        pw-play "${target[@]}" \
            -P '{"application.id":"serpantinum-audio-test","application.name":"Serpantinum audio test","media.role":"music"}' \
            "$TEST_FILE" >/dev/null 2>&1 &
        player_pid=$!
        wait "$player_pid"
    ) &
    echo "$!" > "$TEST_PID_FILE"
}

cmd=$1
arg1=$2
arg2=$3

case $cmd in
    "get")
        cat "$STATE_FILE"
        ;;

    # Vortex: al iniciar sesión (Shell.qml) se vuelve a aplicar el ecualizador guardado, si hay algo que
    # aplicar y EasyEffects está instalado; plano y sin efectos no arranca nada.
    "restaurar")
        command -v easyeffects >/dev/null 2>&1 || exit 0
        jq -e '(.bandas | map(.g | tonumber) | any(. != 0))
               or (.balance | tonumber) != 0 or .mono or .swap or .dynamics != "off"' \
            "$STATE_FILE" >/dev/null 2>&1 || exit 0
        apply_eq
        ;;

    "set_band")
        [[ "$arg1" =~ ^[0-9]+$ && "$arg2" =~ ^-?[0-9]+([.][0-9]+)?$ ]] || exit 1
        updated=$(jq -c --argjson i "$arg1" --argjson val "$arg2" '
            if $i >= 1 and $i <= (.bandas | length)
            then .bandas[$i - 1].g = ([-12, ([12, $val] | min)] | max) | .preset = "Custom" | .pending = true
            else error("no existe esa banda") end' "$STATE_FILE") || exit 1
        echo "$updated" > "$STATE_FILE"
        ;;

    # Vortex 1.10: la lista entera «f:g,f:g,…» (cambiar una frecuencia, agregar o quitar bandas). Con
    # «guardado» queda como ya aplicada (cuando el panel deshace lo que no aplicaste).
    "set_bands")
        [[ "$arg1" =~ ^[0-9]+([.][0-9]+)?:-?[0-9]+([.][0-9]+)?(,[0-9]+([.][0-9]+)?:-?[0-9]+([.][0-9]+)?)*$ ]] || exit 1
        updated=$(jq -c --arg lista "$arg1" --arg guardado "$arg2" --argjson min "$MIN_BANDAS" --argjson max "$MAX_BANDAS" '
            ($lista | split(",") | map(split(":") | {f: ([20, ([20000, (.[0] | tonumber)] | min)] | max | round),
                                                     g: ([-12, ([12, (.[1] | tonumber)] | min)] | max)})
                    | sort_by(.f)) as $b
            | if ($b | length) < $min or ($b | length) > $max then error("de \($min) a \($max) bandas") else . end
            | .bandas = $b
            | if $guardado == "guardado" then .pending = false else .preset = "Custom" | .pending = true end' \
            "$STATE_FILE") || exit 1
        echo "$updated" > "$STATE_FILE"
        ;;

    "apply")
        tmp=$(cat "$STATE_FILE")

        updated=$(echo "$tmp" |
            jq -c ".pending = false")

        echo "$updated" > "$STATE_FILE"

        apply_eq
        ;;

    "set_balance")
        if ! [[ "$arg1" =~ ^-?([0-9]+([.][0-9]*)?|[.][0-9]+)$ ]]; then
            exit 1
        fi

        updated=$(jq -c \
            --argjson val "$arg1" \
            '.balance = (if $val < -1 then -1 elif $val > 1 then 1 else $val end)' \
            "$STATE_FILE") || exit 1

        echo "$updated" > "$STATE_FILE"
        apply_eq
        ;;

    "set_mono"|"set_swap")
        [ "$arg1" = "true" ] || [ "$arg1" = "false" ] || exit 1
        key=${cmd#set_}
        jq -c --arg key "$key" --argjson val "$arg1" '.[$key] = $val' \
            "$STATE_FILE" > "$STATE_FILE.tmp.$$" || exit 1
        mv -f "$STATE_FILE.tmp.$$" "$STATE_FILE"
        apply_eq
        ;;

    "set_dynamics")
        case "$arg1" in off|balanced|night) ;; *) exit 1 ;; esac
        jq -c --arg val "$arg1" '.dynamics = $val' \
            "$STATE_FILE" > "$STATE_FILE.tmp.$$" || exit 1
        mv -f "$STATE_FILE.tmp.$$" "$STATE_FILE"
        apply_eq
        ;;

    "reset_audio")
        jq -c '.balance = 0 | .mono = false | .swap = false | .dynamics = "off"' \
            "$STATE_FILE" > "$STATE_FILE.tmp.$$" || exit 1
        mv -f "$STATE_FILE.tmp.$$" "$STATE_FILE"
        stop_test
        apply_eq
        ;;

    "test")
        play_test
        ;;

    "stop_test")
        stop_test
        ;;

    "preset")
        case $arg1 in
            "Flat")
                save_preset Flat 0 0 0 0 0 0 0 0 0 0
                ;;

            "Bass")
                save_preset Bass 5 7 5 2 1 0 0 0 1 2
                ;;

            "Treble")
                save_preset Treble -2 -1 0 1 2 3 4 5 6 6
                ;;

            "Vocal")
                save_preset Vocal -2 -1 1 3 5 5 4 2 1 0
                ;;

            "Pop")
                save_preset Pop 2 4 2 0 1 2 4 2 1 2
                ;;

            "Rock")
                save_preset Rock 5 4 2 -1 -2 -1 2 4 5 6
                ;;

            "Jazz")
                save_preset Jazz 3 3 1 1 1 1 2 1 2 3
                ;;

            "Classic")
                save_preset Classic 0 1 2 2 2 2 1 2 3 4
                ;;
        esac

        apply_eq
        ;;
esac
