import React from "react";
import { Cpu, ChevronRight, Sparkles, Settings2, Image as ImageIcon, Loader2 } from "lucide-react";
import { motion, AnimatePresence } from "motion/react";
import { cn } from "@sdkwork/ui-mobile-react";
import { AIImageOptions } from "../services/AIImageService";

interface AIImageSettingsPanelProps {
  t: any;
  showModelSelection: boolean;
  setShowModelSelection: (b: boolean) => void;
  selectedModelName: string;
  selectedVendorId: string;
  prompt: string;
  setPrompt: (s: string) => void;
  negativePrompt: string;
  setNegativePrompt: (s: string) => void;
  aspectRatio: AIImageOptions["aspectRatio"];
  setAspectRatio: (s: AIImageOptions["aspectRatio"]) => void;
  ratios: AIImageOptions["aspectRatio"][];
  style: string;
  setStyle: (s: string) => void;
  styles: string[];
  showAdvanced: boolean;
  setShowAdvanced: (b: boolean) => void;
  cfgScale: number;
  setCfgScale: (n: number) => void;
  steps: number;
  setSteps: (n: number) => void;
  seed: string;
  setSeed: (s: string) => void;
  isGenerating: boolean;
  isOptimizingPrompt: boolean;
  handleOptimizePrompt: () => void;
  handleGenerate: () => void;
}

const SliderField = ({ label, min, max, step, value, onChange }: any) => {
  
  return (
  <div className="flex flex-col gap-2 pt-2">
    <div className="flex justify-between items-center text-[13px] text-text-sub">
      <span>{label}</span>
      <span className="font-mono">{value}</span>
    </div>
    <input
      type="range"
      min={min}
      max={max}
      step={step}
      value={value}
      onChange={(e) => onChange(Number(e.target.value))}
      className="w-full h-1.5 bg-gray-200 dark:bg-[#3a3b3c] rounded-lg appearance-none cursor-pointer accent-[#07C160]"
    />
  </div>
);
};


export const AIImageSettingsPanel: React.FC<AIImageSettingsPanelProps> = ({
  t,
  showModelSelection,
  setShowModelSelection,
  selectedModelName,
  selectedVendorId,
  prompt,
  setPrompt,
  negativePrompt,
  setNegativePrompt,
  aspectRatio,
  setAspectRatio,
  ratios,
  style,
  setStyle,
  styles,
  showAdvanced,
  setShowAdvanced,
  cfgScale,
  setCfgScale,
  steps,
  setSteps,
  seed,
  setSeed,
  isGenerating,
  isOptimizingPrompt,
  handleOptimizePrompt,
  handleGenerate,
}) => {
  

return (
    <div className="bg-bg-color p-4 shadow-sm flex flex-col gap-4">
      <div
        onClick={() => setShowModelSelection(true)}
        className="flex items-center justify-between bg-input-bg border border-border-color rounded-xl p-3 cursor-pointer active:bg-active-bg transition-colors"
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-full bg-[#07C160]/10 flex items-center justify-center text-[#07C160]">
            <Cpu className="w-4 h-4" />
          </div>
          <div className="flex flex-col">
            <span className="text-[14px] font-medium text-text-main">{t('settings.model_selection')}</span>
            <span className="text-[12px] text-text-sub">{selectedModelName}</span>
          </div>
        </div>
        <ChevronRight className="w-5 h-5 text-text-sub" />
      </div>

      <div>
        <label className="text-sm font-medium text-text-main flex items-center justify-between mb-2">
          <span>
            {t('settings.prompt_title')} <span className="text-red-500">*</span>
          </span>
        </label>
        <div className="bg-input-bg border border-border-color rounded-2xl p-3 focus-within:border-[#07C160] transition-colors relative">
          <textarea
            className="w-full bg-transparent outline-none resize-none text-[15px] text-text-main min-h-[90px] placeholder-text-sub"
            placeholder={t('settings.prompt_placeholder')}
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
          />
          <button
            onClick={handleOptimizePrompt}
            disabled={isOptimizingPrompt}
            className="absolute bottom-3 right-3 text-[#07C160] bg-[#07C160]/10 p-1.5 rounded-full hover:bg-[#07C160]/20 transition-colors disabled:opacity-40"
            title={t('settings.optimize_prompt')}
          >
            <Sparkles className={cn("w-4 h-4", isOptimizingPrompt && "animate-spin")} />
          </button>
        </div>
      </div>

      <div className="flex gap-2">
        {ratios.map((ratio) => (
          <button
            key={ratio}
            onClick={() => setAspectRatio(ratio)}
            className={`flex-1 py-1.5 rounded-lg border text-[13px] font-medium transition-all ${aspectRatio === ratio ? "border-[#07C160] text-[#07C160] bg-[#07C160]/5" : "border-border-color text-text-sub bg-input-bg"}`}
          >
            {ratio}
          </button>
        ))}
      </div>

      <div>
        <label className="text-sm font-medium text-text-main block mb-2">
          {t('settings.art_style')}
        </label>
        <div className="flex overflow-x-auto no-scrollbar gap-2 pb-1 -mx-2 px-2">
          {styles.map((s) => (
            <button
              key={s}
              onClick={() => setStyle(s)}
              className={`px-4 py-1.5 rounded-full text-[13px] shrink-0 whitespace-nowrap transition-colors ${style === s ? "bg-[#07C160] text-white font-medium" : "bg-input-bg text-text-main border border-border-color active:bg-active-bg"}`}
            >
              {s}
            </button>
          ))}
        </div>
      </div>

      <div className="pt-2 border-t border-border-color">
        <button
          className="flex items-center justify-between w-full text-sm text-text-main font-medium active:opacity-70 transition-opacity pb-1"
          onClick={() => setShowAdvanced(!showAdvanced)}
        >
          <div className="flex items-center gap-1.5">
            <Settings2 className="w-4 h-4 text-[#07C160]" />
            {t('settings.professional_settings')}
          </div>
          <ChevronRight className={cn("w-4 h-4 transition-transform", showAdvanced && "rotate-90")} />
        </button>

        <AnimatePresence>
          {showAdvanced && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              className="overflow-hidden"
            >
              <div className="pt-3 flex flex-col gap-4 pb-2">
                {selectedVendorId !== "openai" && (
                  <div>
                    <label className="text-[13px] text-text-main mb-1.5 block">
                      {t('settings.negative_prompt')}
                    </label>
                    <input
                      type="text"
                      className="w-full bg-input-bg border border-border-color rounded-xl px-3 py-2 text-[14px] text-text-main focus:border-[#07C160] outline-none transition-colors"
                      placeholder={t('settings.negative_prompt_placeholder')}
                      value={negativePrompt}
                      onChange={(e) => setNegativePrompt(e.target.value)}
                    />
                  </div>
                )}
                
                {(selectedVendorId === "stability" || selectedVendorId === "black-forest-labs" || selectedVendorId === "midjourney") && (
                  <>
                    <SliderField label={t('settings.cfg_scale')} min={1} max={20} step={0.5} value={cfgScale} onChange={setCfgScale} />
                    <SliderField label={t('settings.steps')} min={1} max={150} step={1} value={steps} onChange={setSteps} />
                    <div>
                      <label className="text-[13px] text-text-main mb-1.5 block">
                        {t('settings.seed')}
                      </label>
                      <input
                        type="text"
                        className="w-full bg-input-bg border border-border-color rounded-xl px-3 py-2 text-[14px] text-text-main focus:border-[#07C160] outline-none transition-colors"
                        placeholder={t('settings.seed_placeholder')}
                        value={seed}
                        onChange={(e) => setSeed(e.target.value)}
                      />
                    </div>
                  </>
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <button
        disabled={isGenerating || !prompt.trim()}
        onClick={handleGenerate}
        className="w-full h-[46px] rounded-xl bg-[#07C160] text-white font-bold flex items-center justify-center gap-2 disabled:opacity-50 active:scale-[0.98] transition-all shadow-sm shadow-[#07C160]/20"
      >
        {isGenerating ? (
          <Loader2 className="w-5 h-5 animate-spin" />
        ) : (
          <ImageIcon className="w-5 h-5" />
        )}
        {isGenerating ? t('settings.generating') : t('settings.generate_button')}
      </button>
    </div>
  );
};
