-- AlterTable
ALTER TABLE "objects" ALTER COLUMN "type" DROP NOT NULL,
ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- CreateIndex
CREATE INDEX "objects_updated_at_idx" ON "objects"("updated_at" DESC);
