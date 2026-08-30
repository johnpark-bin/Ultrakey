#!/usr/bin/env swift
//
// F-16 / F-14 — 설치된 키보드 입력 소스의 `kTISPropertyInputSourceLanguages` 를 그대로 찍는다.
//
// ⭐ 왜 이 도구가 있는가.
// `korean-input.md` §3.3 은 "한국어 입력기가 활성인가" 를 **언어 프로퍼티의 첫 원소가
// `"ko"` 인가** 로 판정하기로 정했다. 그 판정의 근거는 원래 "사용자의 Karabiner 규칙이
// `input_source_if { language: "ko" }` 로 실제 동작 중" 이라는 **간접 증거**뿐이었다
// (`(추정)`). 이 도구는 그 프로퍼티를 직접 읽어 `(실측)` 으로 승격시킨다.
//
// ⭐ 이 도구는 **시스템 설정을 전혀 바꾸지 않는다.** `TISCreateInputSourceList` 로
// 설치된 소스를 열거해 프로퍼티를 읽기만 한다 — 입력 소스를 전환할 필요도, 특정
// 입력기를 활성화할 필요도 없다. 그래서 검증 절차가 사용자 환경을 건드리지 않는다.
//
// ⚠️ `is_ascii_capable == false` 만으로는 한국어를 구분할 수 없다는 것을 이 도구의
// 출력이 직접 보여준다 — 일본어·중국어 IME 도 전부 `false` 다.
//
// 사용법:
//     swift docs/dev/tools/tis-language-probe.swift            # 전부
//     swift docs/dev/tools/tis-language-probe.swift korean ja  # 부분 문자열 필터
//
// 출력 한 줄에 담기는 것: 입력 소스 ID · ASCII 가능 여부 · 언어 태그 배열(순서 그대로).

import Carbon
import Foundation

let filters = CommandLine.arguments.dropFirst().map { $0.lowercased() }

guard let raw = TISCreateInputSourceList(nil, true)?.takeRetainedValue() as? [TISInputSource] else {
    FileHandle.standardError.write(Data("TISCreateInputSourceList 가 실패했다\n".utf8))
    exit(1)
}

func string(_ src: TISInputSource, _ key: CFString) -> String? {
    guard let p = TISGetInputSourceProperty(src, key) else { return nil }
    return Unmanaged<CFString>.fromOpaque(p).takeUnretainedValue() as String
}

func boolean(_ src: TISInputSource, _ key: CFString) -> Bool? {
    guard let p = TISGetInputSourceProperty(src, key) else { return nil }
    return CFBooleanGetValue(Unmanaged<CFBoolean>.fromOpaque(p).takeUnretainedValue())
}

func languages(_ src: TISInputSource) -> [String] {
    guard let p = TISGetInputSourceProperty(src, kTISPropertyInputSourceLanguages) else { return [] }
    return (Unmanaged<CFArray>.fromOpaque(p).takeUnretainedValue() as? [String]) ?? []
}

for src in raw {
    guard let id = string(src, kTISPropertyInputSourceID) else { continue }
    if !filters.isEmpty && !filters.contains(where: { id.lowercased().contains($0) }) { continue }

    let ascii = boolean(src, kTISPropertyInputSourceIsASCIICapable).map(String.init(describing:)) ?? "?"
    let langs = languages(src)
    // 언어 목록이 아주 긴 소스(ABC 등)가 있어 앞 6개까지만 보인다 — 판정에 쓰는 것은
    // **첫 원소**뿐이므로 그것만 확실히 보이면 된다.
    let shown = langs.prefix(6).joined(separator: ", ")
    let more = langs.count > 6 ? " … (총 \(langs.count)개)" : ""
    print("\(id)")
    print("    asciiCapable = \(ascii)")
    print("    languages[0] = \(langs.first ?? "<없음>")")
    print("    languages    = [\(shown)]\(more)")
}
