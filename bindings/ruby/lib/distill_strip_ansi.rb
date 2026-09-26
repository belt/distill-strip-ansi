require "fiddle"

module DistillStripAnsi
  ABI_VERSION = 1
  ERROR_INVALID_ARGUMENT = -1

  module Native
    def self.library_candidates
      configured = ENV["DISTILL_STRIP_ANSI_LIB"]
      candidates = configured ? [configured] : []
      names = case RUBY_PLATFORM
              when /darwin/
                ["libdistill_strip_ansi_c.dylib"]
              when /mswin|mingw/
                ["distill_strip_ansi_c.dll"]
              else
                ["libdistill_strip_ansi_c.so"]
              end
      project_root = File.expand_path("../../..", __dir__)
      %w[release debug].each do |profile|
        names.each { |name| candidates << File.join(project_root, "target", profile, name) }
      end
      candidates.find { |candidate| File.file?(candidate) } || names.first
    end

    def self.load
      handle = Fiddle::Handle.new(library_candidates)
      abi_version = Fiddle::Function.new(handle["dsa_abi_version"], [], Fiddle::TYPE_INT)
      strip = Fiddle::Function.new(
        handle["dsa_strip"],
        [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T, Fiddle::TYPE_VOIDP],
        Fiddle::TYPE_INTPTR_T
      )
      contains = Fiddle::Function.new(
        handle["dsa_contains_ansi"],
        [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T],
        Fiddle::TYPE_INT
      )
      [handle, abi_version, strip, contains]
    end

    HANDLE, ABI_VERSION_FUNCTION, STRIP_FUNCTION, CONTAINS_FUNCTION = load
  end

  unless Native::ABI_VERSION_FUNCTION.call == ABI_VERSION
    raise LoadError, "unsupported distill-strip-ansi C ABI version"
  end

  def self.strip(input)
    bytes = String(input).b
    return "" if bytes.empty?

    output = Fiddle::Pointer.malloc(bytes.bytesize)
    count = Native::STRIP_FUNCTION.call(Fiddle::Pointer[bytes], bytes.bytesize, output)
    raise ArgumentError, "invalid argument passed to distill-strip-ansi" if count == ERROR_INVALID_ARGUMENT
    raise RuntimeError, "distill-strip-ansi failed with status #{count}" if count.negative?

    output[0, count]
  end

  def self.contains_ansi?(input)
    bytes = String(input).b
    return false if bytes.empty?

    result = Native::CONTAINS_FUNCTION.call(Fiddle::Pointer[bytes], bytes.bytesize)
    raise ArgumentError, "invalid argument passed to distill-strip-ansi" if result.negative?

    result == 1
  end
end