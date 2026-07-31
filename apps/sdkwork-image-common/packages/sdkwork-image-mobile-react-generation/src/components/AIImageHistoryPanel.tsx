import React from "react";
import { Image as ImageIcon, Trash2, Download, Loader2 } from "lucide-react";
import { motion, AnimatePresence } from "motion/react";
import { cn } from "@sdkwork/ui-mobile-react";
import { ImageTask, AIImageOptions } from "../services/AIImageService";
import { AIImageHistoryCard } from "./AIImageHistoryCard";

interface AIImageHistoryPanelProps {
  t: any;
  currentTask: ImageTask | null;
  history: ImageTask[];
  isGenerating: boolean;
  currentProgress: number;
  downloadImage: (url?: string) => void;
  handleDelete: (e: React.MouseEvent, id: string) => void;
  setPrompt: (s: string) => void;
  setAspectRatio: (s: AIImageOptions["aspectRatio"]) => void;
  setStyle: (s: string) => void;
  setCurrentTask: (t: ImageTask | null) => void;
}

export const AIImageHistoryPanel: React.FC<AIImageHistoryPanelProps> = ({
  t,
  currentTask,
  history,
  isGenerating,
  currentProgress,
  downloadImage,
  handleDelete,
  setPrompt,
  setAspectRatio,
  setStyle,
  setCurrentTask,
}) => {
  

return (
    <div className="px-4 pb-6">
      <AnimatePresence>
        {currentTask && (
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            className="flex flex-col gap-3 mb-6"
          >
            <div className="flex justify-between items-center bg-bg-color px-4 py-2.5 rounded-t-xl border-x border-t border-border-color shadow-sm -mb-4 z-10 relative">
              <h3 className="text-[15px] font-bold text-text-main bg-gradient-to-r from-[#07C160] to-teal-500 bg-clip-text text-transparent">
                {t('result.title')}
              </h3>
              {currentTask.status === "completed" && (
                <button
                  onClick={() => {
                    downloadImage(currentTask.imageUrl);
                  }}
                  className="text-[#576B95] text-sm font-medium flex items-center gap-1 active:opacity-70 bg-[#576B95]/10 px-3 py-1.5 rounded-lg border border-[#576B95]/20 shadow-sm"
                >
                  <Download className="w-3.5 h-3.5" /> {t('result.save')}
                </button>
              )}
            </div>

            <div
              className={cn(
                "bg-input-bg rounded-b-xl rounded-t-sm overflow-hidden border border-border-color relative flex items-center justify-center mx-auto shadow-md",
                currentTask.options.aspectRatio === "16:9"
                  ? "w-full aspect-video"
                  : currentTask.options.aspectRatio === "9:16"
                    ? "w-[75%] aspect-[9/16]"
                    : currentTask.options.aspectRatio === "4:3"
                      ? "w-full aspect-[4/3]"
                      : "w-full aspect-square",
              )}
            >
              {currentTask.status === "generating" ? (
                <div className="flex flex-col items-center justify-center text-text-sub w-full h-full p-8 absolute inset-0 bg-bg-color/50 backdrop-blur-md">
                  <Loader2 className="w-10 h-10 animate-spin mb-4 text-[#07C160]" />
                  <div className="w-[120px] max-w-full h-1.5 bg-border-color rounded-full overflow-hidden mb-2">
                    <div
                      className="h-full bg-[#07C160] transition-all duration-300"
                      style={{ width: `${currentProgress}%` }}
                    />
                  </div>
                  <span className="text-[13px] font-medium text-[#07C160]">
                    {currentProgress}%
                  </span>
                </div>
              ) : (
                <img
                  src={currentTask.imageUrl}
                  alt="Generated Result"
                  className="w-full h-full object-cover"
                />
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {!isGenerating && (
        <div className="flex flex-col gap-3">
          {history.length > 0 ? (
            <>
              <h3
                id="history-section"
                className="text-[16px] font-bold text-text-main pb-1"
              >
                {t('history.title')}
              </h3>
              <div className="grid grid-cols-2 lg:grid-cols-3 gap-3">
                {history.map((item) => (
                  <AIImageHistoryCard
                    key={item.id}
                    item={item}
                    onSelect={(selected) => {
                      setPrompt(selected.options.prompt);
                      setAspectRatio(selected.options.aspectRatio);
                      setStyle(selected.options.style);
                      setCurrentTask(selected);
                    }}
                    onDelete={handleDelete}
                  />
                ))}
              </div>
            </>
          ) : !currentTask ? (
            <div className="pt-6 flex flex-col items-center justify-center opacity-70">
              <ImageIcon className="w-12 h-12 text-text-sub mb-3 opacity-50" />
              <h3 className="text-sm font-medium text-text-sub mb-4">
                {t('history.empty')}
              </h3>
              <div className="flex flex-wrap gap-2 justify-center px-4">
                {[
                  "A cyberpunk city with flying cars",
                  "A cute cat wearing sunglasses",
                  "A magical glowing forest",
                  "A cozy cabin in the snow",
                  "An astronaut on Mars",
                ].map((suggestion, i) => (
                  <button
                    key={i}
                    onClick={() => setPrompt(suggestion)}
                    className="bg-active-bg border border-border-color px-3 py-1.5 rounded-full text-xs text-text-main hover:border-[#07C160] transition-colors active:scale-95"
                  >
                    {suggestion}
                  </button>
                ))}
              </div>
            </div>
          ) : null}
        </div>
      )}
    </div>
  );
};
