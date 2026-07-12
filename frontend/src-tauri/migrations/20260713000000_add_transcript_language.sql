-- Per-sentence language, as detected by the engine (SenseVoice reports it; Whisper and
-- Parakeet do not, and leave it NULL).
--
-- Without this column the language was known only while recording and lost the moment
-- the meeting was saved, so re-opening a bilingual meeting showed no language at all.
ALTER TABLE transcripts ADD COLUMN language TEXT;
